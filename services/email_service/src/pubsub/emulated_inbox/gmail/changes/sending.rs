//! The owner sending mail from Macro: durable send admission queues it, and
//! the scheduled delivery worker sends it through Gmail.

use super::super::super::delivery::{self, Outgoing};
use super::{GmailInboxInMacro, RECIPIENT};
use crate::pubsub::emulated_inbox::gmail::OWNER;
use crate::pubsub::test_support::fake_sqs;
use email::domain::send_attempt::SendAttemptStatus;
use models_email::service::pubsub::ScheduledPubsubMessage;
use uuid::Uuid;

impl GmailInboxInMacro {
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

    /// Time passes until every scheduled send and every delivery retry is
    /// due, the scheduler hands them to the scheduled queue, as its scan
    /// does, and the workers run.
    pub async fn time_passes_and_the_scheduler_runs(&mut self) {
        let due: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE email_scheduled_messages
             SET send_time = LEAST(send_time, now()),
                 delivery_lease_expires_at = LEAST(delivery_lease_expires_at, now())
             WHERE link_id = $1 AND NOT sent
             RETURNING message_id",
        )
        .bind(self.link_id)
        .fetch_all(&self.db)
        .await
        .unwrap();
        let sqs_client = sqs_client::SQS::new(fake_sqs::client(&self.queues.sqs_url(), false))
            .email_scheduled_queue(&self.scheduled_queue);
        for message_id in due {
            sqs_client
                .enqueue_email_scheduled_message(
                    ScheduledPubsubMessage {
                        link_id: self.link_id,
                        message_id,
                    },
                    None,
                )
                .await
                .unwrap();
        }
        self.changes_reach_gmail().await;
    }

    /// What Macro tells the owner about the send.
    pub async fn send_status(&self, outgoing: &Outgoing) -> SendAttemptStatus {
        delivery::send_status(&self.email, OWNER, self.link_id, outgoing).await
    }

    /// Whether Macro still keeps the sent message as a draft.
    pub async fn macro_keeps_a_draft(&self, outgoing: &Outgoing) -> bool {
        delivery::keeps_a_draft(&self.db, outgoing).await
    }

    /// Copies of `outgoing` in the recipient's Gmail inbox.
    pub fn recipient_copies(&self, outgoing: &Outgoing) -> usize {
        self.copies(RECIPIENT, outgoing, "INBOX")
    }

    /// Copies of `outgoing` in the owner's Gmail Sent.
    pub fn sent_copies(&self, outgoing: &Outgoing) -> usize {
        self.copies(OWNER, outgoing, "SENT")
    }

    /// Gmail drafts of `outgoing` left in the owner's mailbox.
    pub fn gmail_drafts(&self, outgoing: &Outgoing) -> usize {
        self.copies(OWNER, outgoing, "DRAFT")
    }

    fn copies(&self, mailbox: &str, outgoing: &Outgoing, label: &str) -> usize {
        self.gmail
            .messages(mailbox)
            .expect("the mailbox")
            .into_iter()
            .filter(|message| message.subject.as_deref() == Some(outgoing.subject.as_str()))
            .filter(|message| message.labels.iter().any(|held| held == label))
            .count()
    }
}
