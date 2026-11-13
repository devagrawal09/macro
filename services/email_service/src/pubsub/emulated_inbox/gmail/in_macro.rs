//! The owner's changes in Macro reaching Gmail.
//!
//! The owner acts through the email domain service the HTTP and GraphQL
//! routes call: label changes go through the label-write service onto the
//! gmail_ops queues, and sends through durable send admission onto the
//! scheduled queue. The production gmail_ops and scheduled delivery workers
//! then take them to the emulated Gmail. Those queues live on an SQS
//! endpoint of their own, so the inbox's sync delivery never sees them.

#![expect(
    clippy::disallowed_methods,
    reason = "harness SQL reads and moves test state; it stays out of the offline query cache"
)]

use super::super::delivery;
use super::super::queues::{QueueSpec, Queues};
use super::{GmailInbox, OWNER, Received};
use crate::pubsub::gmail_ops::worker::GmailOpsContext;
use crate::pubsub::scheduled::context::ScheduledContext;
use crate::pubsub::test_support::{self, GmailEndpoints, fake_sqs};
use email::domain::models::mailbox_action::MailboxAction;
use email::domain::ports::EmailService;
use gmail_emulator::SeedMailbox;
use macro_event_broker::{KafkaEventPublisher, MacroEventBrokerService};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use tokio_util::task::TaskTracker;
use uuid::Uuid;

mod sending;
#[cfg(test)]
mod specs;

/// Someone else the owner writes to, with a mailbox in the same Gmail.
pub const RECIPIENT: &str = "lee@example.com";

/// The queues' redrive policies (`infra/stacks/email-service`; the
/// scheduled queue keeps the default of five).
const OPS_MAX_RECEIVES: u32 = 3;
const OPS_RETRY_MAX_RECEIVES: u32 = 100;
const SCHEDULED_MAX_RECEIVES: u32 = 5;

/// More deliveries than any spec needs; work that doesn't settle by then is
/// looping.
const MAX_DELIVERIES: usize = 500;

/// A Gmail inbox the owner also uses in Macro.
pub struct GmailInboxInMacro {
    inbox: GmailInbox,
    email: delivery::EmailSvc,
    queues: Queues,
    ops_queue: String,
    ops_retry_queue: String,
    scheduled_queue: String,
    ops: GmailOpsContext,
    ops_retry: GmailOpsContext,
    scheduled: ScheduledContext,
}

impl Deref for GmailInboxInMacro {
    type Target = GmailInbox;

    fn deref(&self) -> &GmailInbox {
        &self.inbox
    }
}

impl DerefMut for GmailInboxInMacro {
    fn deref_mut(&mut self) -> &mut GmailInbox {
        &mut self.inbox
    }
}

impl GmailInboxInMacro {
    /// A connected inbox, and a second Gmail mailbox for the recipient.
    pub async fn connected(db: sqlx::PgPool) -> GmailInboxInMacro {
        let inbox = GmailInbox::connected_to(db.clone(), SeedMailbox::new(OWNER)).await;
        inbox
            .gmail
            .add_mailbox(&SeedMailbox::new(RECIPIENT))
            .expect("the recipient's mailbox");

        let services_url = super::super::services::start().await;
        let sqs = fake_sqs::FakeSqs::start().await;
        let sqs_sdk = fake_sqs::client(&sqs.url, false);
        let ops_queue = sqs.queue_url("gmail-ops");
        let ops_retry_queue = sqs.queue_url("gmail-ops-retry");
        let scheduled_queue = sqs.queue_url("scheduled");
        let sqs_client = sqs_client::SQS::new(sqs_sdk.clone())
            .gmail_ops_queue(&ops_queue)
            .gmail_ops_retry_queue(&ops_retry_queue)
            .email_scheduled_queue(&scheduled_queue)
            .email_backfill_queue(&sqs.queue_url("backfill"))
            .email_link_manager_queue(&sqs.queue_url("link-manager"))
            .sfs_uploader_queue(&sqs.queue_url("sfs"));
        let redis = redis::Client::open(test_support::redis_url()).expect("redis url");
        let email_api = test_support::gmail_api(
            db.clone(),
            &redis,
            GmailEndpoints {
                gmail: &inbox.gmail.gmail_url(),
                people: &inbox.gmail.people_url(),
                certs: &inbox.gmail.certs_url(),
                auth: &inbox.gmail.authentication_service_url(),
            },
            sqs_client.clone(),
        )
        .await;

        let worker =
            |queue: &str| sqs_worker::SQSWorker::new(sqs_sdk.clone(), queue.to_string(), 1, 1);
        let ops = |retry_worker: bool, queue: &str| {
            crate::pubsub::gmail_ops::worker::context(
                db.clone(),
                &worker(queue),
                sqs_client.clone(),
                email_api.clone(),
                retry_worker,
            )
        };
        let scheduled = crate::pubsub::scheduled::worker::context(
            worker(&scheduled_queue),
            db.clone(),
            email_api.clone(),
            delivery::draft_content(
                &db,
                &services_url,
                email_api.clone(),
                delivery::outlook_api(&db, &services_url, &services_url),
            ),
            delivery::s3(&services_url),
            delivery::bucket(),
            MacroEventBrokerService::new(
                KafkaEventPublisher::new("127.0.0.1:9092").expect("kafka publisher"),
                TaskTracker::new(),
            ),
            delivery::read_receipts(&db, &services_url),
        );
        let email = delivery::email_service(&db, sqs_client.clone());
        let queues = Queues::new(
            sqs,
            vec![
                QueueSpec {
                    url: ops_queue.clone(),
                    max_receives: OPS_MAX_RECEIVES,
                },
                QueueSpec {
                    url: ops_retry_queue.clone(),
                    max_receives: OPS_RETRY_MAX_RECEIVES,
                },
                QueueSpec {
                    url: scheduled_queue.clone(),
                    max_receives: SCHEDULED_MAX_RECEIVES,
                },
            ],
        );

        GmailInboxInMacro {
            ops: ops(false, &ops_queue),
            ops_retry: ops(true, &ops_retry_queue),
            inbox,
            email,
            queues,
            ops_queue,
            ops_retry_queue,
            scheduled_queue,
            scheduled,
        }
    }

    /// The owner marks the thread of `received` read in Macro.
    pub async fn owner_reads(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Read(true))
            .await;
    }

    /// The owner archives the thread of `received` in Macro.
    pub async fn owner_archives(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Archived(true))
            .await;
    }

    /// The owner stars the thread of `received` in Macro.
    pub async fn owner_stars(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Flagged(true))
            .await;
    }

    /// The owner moves the thread of `received` to the trash in Macro.
    pub async fn owner_trashes(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Trashed(true))
            .await;
    }

    /// The owner marks `received` read in Gmail itself.
    pub fn owner_reads_in_gmail(&self, received: &Received) {
        self.gmail
            .modify_labels(OWNER, &received.id, &[], &["UNREAD".to_string()])
            .expect("mark read in Gmail");
    }

    /// The gmail_ops and scheduled workers run until nothing is left to
    /// deliver.
    pub async fn changes_reach_gmail(&mut self) {
        for _ in 0..MAX_DELIVERIES {
            self.queues.absorb();
            let Some((queued, message)) = self.queues.receive() else {
                return;
            };
            if queued.queue_url == self.ops_queue {
                crate::pubsub::gmail_ops::process::process_message(self.ops.clone(), &message)
                    .await
                    .ok();
            } else if queued.queue_url == self.ops_retry_queue {
                crate::pubsub::gmail_ops::process::process_message(
                    self.ops_retry.clone(),
                    &message,
                )
                .await
                .ok();
            } else {
                crate::pubsub::scheduled::process::process_message(
                    self.scheduled.clone(),
                    &message,
                )
                .await
                .ok();
            }
            self.queues.settle(queued);
        }
        panic!("gmail_ops and scheduled delivery did not settle");
    }

    /// The labels Gmail holds on `received`.
    pub fn gmail_labels(&self, received: &Received) -> BTreeSet<String> {
        self.gmail
            .messages(OWNER)
            .expect("the owner's mailbox")
            .into_iter()
            .find(|message| message.id == received.id)
            .map(|message| message.labels.into_iter().collect())
            .unwrap_or_default()
    }

    /// The labels Macro shows on `received`.
    pub async fn macro_labels(&self, received: &Received) -> BTreeSet<String> {
        sqlx::query_scalar(
            "SELECT l.provider_label_id FROM email_messages m
             JOIN email_message_labels ml ON ml.message_id = m.id
             JOIN email_labels l ON l.id = ml.label_id
             WHERE m.link_id = $1 AND m.provider_id = $2",
        )
        .bind(self.link_id)
        .bind(&received.id)
        .fetch_all(&self.db)
        .await
        .unwrap()
        .into_iter()
        .collect()
    }

    /// Work SQS gave up on, in the sync queues and in these.
    pub fn all_dead_letters(&self) -> Vec<String> {
        self.inbox
            .dead_letters()
            .iter()
            .chain(&self.queues.dead_letters)
            .map(|queued| format!("{}: {}", queued.queue_url, queued.body))
            .collect()
    }

    async fn owner_changes(&self, received: &Received, action: MailboxAction) {
        let thread: Uuid = sqlx::query_scalar(
            "SELECT thread_id FROM email_messages WHERE link_id = $1 AND provider_id = $2",
        )
        .bind(self.link_id)
        .bind(&received.id)
        .fetch_one(&self.db)
        .await
        .expect("Macro shows the mail");
        self.email
            .change_thread_mailbox_state(delivery::macro_user(OWNER), thread, action)
            .await
            .expect("the change is accepted");
    }
}
