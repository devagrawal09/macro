//! The owner's changes and sends in Macro reaching Outlook.
//!
//! The owner acts through the email domain service the HTTP and GraphQL
//! routes call: an action is stored as a mailbox command, and a send as a
//! durable draft. The production command and draft loops
//! (`mailbox_sync_worker::run_commands`, `run_drafts`) then take them to the
//! emulated Outlook.

#![expect(
    clippy::disallowed_methods,
    reason = "harness SQL reads and moves test state; it stays out of the offline query cache"
)]

use super::super::delivery::{self, Outgoing};
use super::{OWNER, OutlookInbox, Received};
use crate::pubsub::test_support::{self, GmailEndpoints, fake_sqs};
use email::domain::mailbox::commands::MailboxCommandService;
use email::domain::mailbox::drafts::MailboxDraftService;
use email::domain::models::mailbox_action::MailboxAction;
use email::domain::ports::EmailService;
use email::domain::read_receipts::ReadReceiptMaterializer;
use email::domain::send_attempt::SendAttemptStatus;
use email::outbound::mailbox_pg::PgMailboxSync;
use email::outbound::read_receipts_pg::ReadReceiptPgRepository;
use outlook_emulator::{MessageSummary, SeedMailbox};
use std::ops::Deref;
use uuid::Uuid;

#[cfg(test)]
mod specs;

/// Someone else the owner writes to, with a mailbox in the same Outlook.
pub const RECIPIENT: &str = "lee@example.com";

/// More passes than any spec needs; work that doesn't settle by then is
/// looping.
const MAX_PASSES: usize = 200;

/// What Macro shows for a message.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Shown {
    pub read: bool,
    pub starred: bool,
    pub in_inbox: bool,
    pub in_trash: bool,
}

/// An Outlook inbox the owner also uses in Macro.
pub struct OutlookInboxInMacro {
    inbox: OutlookInbox,
    email: delivery::EmailSvc,
    commands: MailboxCommandService<PgMailboxSync, delivery::OutlookGateway>,
    drafts: MailboxDraftService<
        PgMailboxSync,
        delivery::OutlookGateway,
        ReadReceiptMaterializer<delivery::DraftContent, ReadReceiptPgRepository>,
    >,
}

impl Deref for OutlookInboxInMacro {
    type Target = OutlookInbox;

    fn deref(&self) -> &OutlookInbox {
        &self.inbox
    }
}

impl OutlookInboxInMacro {
    /// A connected inbox, and a second Outlook mailbox for the recipient.
    pub async fn connected(db: sqlx::PgPool) -> OutlookInboxInMacro {
        let inbox = OutlookInbox::connected(db.clone()).await;
        inbox
            .outlook
            .add_mailbox(&SeedMailbox::new(RECIPIENT))
            .expect("the recipient's mailbox");

        // Outlook sends don't go through SQS; what the domain queues anyway
        // is dropped.
        let services_url = super::super::services::start().await;
        let sqs = fake_sqs::FakeSqs::start().await;
        let sqs_client = sqs_client::SQS::new(fake_sqs::client(&sqs.url, false))
            .email_scheduled_queue(&sqs.queue_url("scheduled"))
            .gmail_ops_queue(&sqs.queue_url("gmail-ops"))
            .sfs_uploader_queue(&sqs.queue_url("sfs"));
        let outlook_api = || {
            delivery::outlook_api(
                &db,
                &inbox.outlook.graph_url(),
                &inbox.outlook.authentication_service_url(),
            )
        };
        let unused_gmail_api = test_support::gmail_api(
            db.clone(),
            &redis::Client::open(test_support::redis_url()).expect("redis url"),
            GmailEndpoints {
                gmail: &services_url,
                people: &services_url,
                certs: &services_url,
                auth: &services_url,
            },
            sqs_client.clone(),
        )
        .await;
        let repository = PgMailboxSync::new(db.clone());

        OutlookInboxInMacro {
            email: delivery::email_service(&db, sqs_client),
            commands: MailboxCommandService::new(
                repository.clone(),
                crate::outbound::email_api::ProviderMailboxGateway(outlook_api()),
            ),
            drafts: MailboxDraftService::new(
                repository,
                crate::outbound::email_api::ProviderMailboxGateway(outlook_api()),
                ReadReceiptMaterializer::new(
                    delivery::draft_content(&db, &services_url, unused_gmail_api, outlook_api()),
                    delivery::read_receipts(&db, &services_url),
                ),
            ),
            inbox,
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

    /// The owner flags the thread of `received` in Macro.
    pub async fn owner_flags(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Flagged(true))
            .await;
    }

    /// The owner moves the thread of `received` to the trash in Macro.
    pub async fn owner_trashes(&self, received: &Received) {
        self.owner_changes(received, MailboxAction::Trashed(true))
            .await;
    }

    /// The owner sends `subject` to the recipient from Macro.
    pub async fn owner_sends(&self, subject: &str) -> Outgoing {
        delivery::admit_send(&self.email, OWNER, self.link_id, RECIPIENT, subject, None).await
    }

    /// The owner schedules `subject` to the recipient for an hour from now.
    pub async fn owner_schedules(&self, subject: &str) -> Outgoing {
        let in_an_hour = chrono::Utc::now() + chrono::Duration::hours(1);
        delivery::admit_send(
            &self.email,
            OWNER,
            self.link_id,
            RECIPIENT,
            subject,
            Some(in_an_hour),
        )
        .await
    }

    /// The command and draft loops run, and the few seconds each step
    /// waits before the next pass go by, until neither loop has work.
    pub async fn changes_reach_outlook(&self) {
        for _ in 0..MAX_PASSES {
            // A failed claim is rescheduled with backoff, as the worker loop
            // leaves it, so it counts as no work done in this pass.
            let commanded = self.commands.execute_once().await.unwrap_or(false);
            let drafted = self.drafts.execute_once().await.unwrap_or(false);
            if !commanded && !drafted && !self.a_few_seconds_pass().await {
                return;
            }
        }
        panic!("the Outlook command and draft loops did not settle");
    }

    /// Half a minute passes for work that waits that long or less, which
    /// becomes due; whether there was any. The draft loop's clock for a
    /// stage, stored in its checkpoint, moves along with it.
    async fn a_few_seconds_pass(&self) -> bool {
        let mut waited = 0;
        for statement in [
            "UPDATE email_mailbox_commands SET available_at = now()
             WHERE link_id = $1 AND available_at > now()
                AND available_at <= now() + interval '30 seconds'",
            "UPDATE email_mailbox_drafts SET available_at = now(),
                checkpoint = CASE WHEN checkpoint ? 'stage_started_at' THEN jsonb_set(checkpoint,
                    '{stage_started_at}',
                    to_jsonb((checkpoint->>'stage_started_at')::timestamptz
                        - interval '30 seconds'))
                ELSE checkpoint END
             WHERE link_id = $1 AND available_at > now()
                AND available_at <= now() + interval '30 seconds'",
        ] {
            waited += sqlx::query(statement)
                .bind(self.link_id)
                .execute(&self.db)
                .await
                .unwrap()
                .rows_affected();
        }
        waited > 0
    }

    /// Time passes: scheduled sends come due, every retry's backoff ends,
    /// and the draft loop's ten-minute wait for a send it couldn't confirm
    /// runs out. Then the loops run again.
    pub async fn time_passes(&self) {
        for statement in [
            "UPDATE email_scheduled_messages SET send_time = LEAST(send_time, now())
             WHERE link_id = $1 AND NOT sent",
            "UPDATE email_mailbox_commands SET available_at = now() WHERE link_id = $1",
            "UPDATE email_mailbox_drafts SET available_at = now(),
                checkpoint = CASE WHEN checkpoint ? 'stage_started_at' THEN jsonb_set(checkpoint,
                    '{stage_started_at}',
                    to_jsonb((checkpoint->>'stage_started_at')::timestamptz
                        - interval '11 minutes'))
                ELSE checkpoint END
             WHERE link_id = $1",
        ] {
            sqlx::query(statement)
                .bind(self.link_id)
                .execute(&self.db)
                .await
                .unwrap();
        }
        self.changes_reach_outlook().await;
    }

    /// The message as Outlook holds it.
    pub fn in_outlook(&self, received: &Received) -> MessageSummary {
        self.outlook
            .messages(OWNER)
            .expect("the owner's mailbox")
            .into_iter()
            .find(|message| message.id == received.id)
            .expect("Outlook holds the message")
    }

    /// What Macro shows for `received`.
    pub async fn macro_shows(&self, received: &Received) -> Shown {
        sqlx::query_as(
            "SELECT m.is_read AS read, m.is_starred AS starred,
                COALESCE(f.in_inbox, false) AS in_inbox, COALESCE(f.in_trash, false) AS in_trash
             FROM email_messages m JOIN email_message_mailbox_facts f ON f.id = m.id
             WHERE m.link_id = $1 AND m.provider_id = $2",
        )
        .bind(self.link_id)
        .bind(&received.id)
        .fetch_one(&self.db)
        .await
        .expect("Macro shows the mail")
    }

    /// What Macro tells the owner about the send.
    pub async fn send_status(&self, outgoing: &Outgoing) -> SendAttemptStatus {
        delivery::send_status(&self.email, OWNER, self.link_id, outgoing).await
    }

    /// Whether Macro still keeps the sent message as a draft.
    pub async fn macro_keeps_a_draft(&self, outgoing: &Outgoing) -> bool {
        delivery::keeps_a_draft(&self.db, outgoing).await
    }

    /// Copies of `outgoing` in the recipient's Outlook inbox.
    pub fn recipient_copies(&self, outgoing: &Outgoing) -> usize {
        self.copies(RECIPIENT, outgoing)
            .filter(|message| message.folder == "Inbox")
            .count()
    }

    /// Copies of `outgoing` in the owner's Sent Items.
    pub fn sent_copies(&self, outgoing: &Outgoing) -> usize {
        self.copies(OWNER, outgoing)
            .filter(|message| message.folder == "Sent Items" && !message.is_draft)
            .count()
    }

    /// Outlook drafts of `outgoing` left in the owner's mailbox.
    pub fn outlook_drafts(&self, outgoing: &Outgoing) -> usize {
        self.copies(OWNER, outgoing)
            .filter(|message| message.is_draft)
            .count()
    }

    fn copies(&self, mailbox: &str, outgoing: &Outgoing) -> impl Iterator<Item = MessageSummary> {
        let subject = outgoing.subject.clone();
        self.outlook
            .messages(mailbox)
            .expect("the mailbox")
            .into_iter()
            .filter(move |message| message.subject == subject)
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
