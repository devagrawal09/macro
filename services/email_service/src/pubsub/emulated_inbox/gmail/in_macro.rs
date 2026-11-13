//! The owner's changes in Macro reaching Gmail.
//!
//! The owner acts through the email domain service the HTTP and GraphQL
//! routes call: label changes go through the label-write service onto the
//! gmail_ops queues, and sends through durable send admission onto the
//! scheduled queue. The production gmail_ops and scheduled delivery workers
//! then take them to the emulated Gmail. Those queues live on an SQS
//! endpoint of their own, so the inbox's sync delivery never sees them.

use super::super::queues::{QueueSpec, Queues};
use super::{GmailInbox, OWNER, Received};
use crate::pubsub::gmail_ops::worker::GmailOpsContext;
use crate::pubsub::scheduled::context::ScheduledContext;
use crate::pubsub::test_support::{self, GmailEndpoints, fake_sqs};
use email::domain::mailbox::credentials::MailboxCredentials;
use email::domain::models::mailbox_action::MailboxAction;
use email::domain::ports::EmailService;
use email::domain::service::EmailServiceImpl;
use email::outbound::EmailPgRepo;
use email::outbound::mailbox_pg::PgMailboxSync;
use frecency::domain::services::FrecencyQueryServiceImpl;
use frecency::outbound::postgres::FrecencyPgStorage;
use gmail_emulator::SeedMailbox;
use macro_event_broker::{KafkaEventPublisher, MacroEventBrokerService};
use macro_user_id::user_id::MacroUserIdStr;
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
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

type EmailSvc = EmailServiceImpl<
    EmailPgRepo,
    FrecencyQueryServiceImpl<FrecencyPgStorage>,
    sqs_client::SQS,
    crm::domain::service::CrmServiceImpl<
        crm::outbound::companies_repo::CompaniesRepositoryImpl,
        crm::outbound::no_op_resolver::NoOpCompanyMetadataResolver,
    >,
    entity_access_management::domain::service::EntityAccessManagementServiceImpl<
        entity_access_management::outbound::PgRepository,
    >,
>;

/// A Gmail inbox the owner also uses in Macro.
pub struct GmailInboxInMacro {
    inbox: GmailInbox,
    email: EmailSvc,
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
        let s3 = s3_client::S3::new(s3(&services_url));
        let draft_content = Arc::new(
            email::domain::mailbox::drafts::content::DraftContentService {
                repository: EmailPgRepo::new(db.clone()),
                bytes: crate::outbound::mailbox_drafts::MailboxDraftContent {
                    provider: crate::outbound::attachment_access::ProviderAttachmentBytes {
                        s3: s3.clone(),
                        bucket: "attachments".to_string(),
                        gmail: email_api.clone(),
                        outlook: crate::outbound::email_api::ProviderMailboxGateway(
                            unused_outlook_api(&db, &services_url),
                        ),
                    },
                    s3: s3.clone(),
                    bucket: "attachments".to_string(),
                },
                access: entity_access::domain::service::EntityAccessServiceImpl::new(
                    entity_access::outbound::PgAccessRepository::new(db.clone()),
                ),
            },
        );
        let scheduled = crate::pubsub::scheduled::worker::context(
            worker(&scheduled_queue),
            db.clone(),
            email_api.clone(),
            draft_content,
            s3,
            "attachments".to_string(),
            MacroEventBrokerService::new(
                KafkaEventPublisher::new("127.0.0.1:9092").expect("kafka publisher"),
                TaskTracker::new(),
            ),
            email::domain::read_receipts::ReadReceiptService::new(
                email::outbound::read_receipts_pg::ReadReceiptPgRepository(db.clone()),
                email::domain::read_receipts::PixelUrlPrefix::new(&services_url),
            ),
        );
        let email = EmailServiceImpl::new(
            EmailPgRepo::new(db.clone()),
            FrecencyQueryServiceImpl::new(FrecencyPgStorage::new(db.clone())),
            sqs_client.clone(),
            crm::domain::service::CrmServiceImpl::new(
                crm::outbound::companies_repo::CompaniesRepositoryImpl::new(db.clone()),
                crm::outbound::no_op_resolver::NoOpCompanyMetadataResolver,
            ),
            entity_access_management::domain::service::EntityAccessManagementServiceImpl::new(
                entity_access_management::outbound::PgRepository::new(db.clone()),
            ),
            0,
        );
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
            .change_thread_mailbox_state(owner(), thread, action)
            .await
            .expect("the change is accepted");
    }
}

fn owner() -> MacroUserIdStr<'static> {
    MacroUserIdStr::try_from_email(OWNER).unwrap()
}

/// An S3 client for the stand-in services; no spec sends attachments.
fn s3(services_url: &str) -> aws_sdk_s3::Client {
    aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                "test", "test", None, None, "harness",
            ))
            .endpoint_url(services_url)
            .build(),
    )
}

/// The Outlook half of the attachment reader, which Gmail sends never call.
fn unused_outlook_api(
    db: &sqlx::PgPool,
    services_url: &str,
) -> email_api_client::domain::service::mailbox::MailboxApiService<
    email_api_client::OutlookApiClientRepository,
    MailboxCredentials<PgMailboxSync, crate::outbound::email_api::MicrosoftCredentialsClient>,
    email_api_client::domain::ports::AlwaysAllowRateLimiter,
> {
    email_api_client::domain::service::mailbox::MailboxApiService::new(
        email_api_client::OutlookApiClientRepository::new().expect("outlook client"),
        MailboxCredentials::new(
            PgMailboxSync::new(db.clone()),
            crate::outbound::email_api::MicrosoftCredentialsClient(
                authentication_service_client::AuthServiceClient::new(
                    "secret".to_string(),
                    services_url.to_string(),
                ),
            ),
        ),
        email_api_client::domain::ports::AlwaysAllowRateLimiter,
    )
}
