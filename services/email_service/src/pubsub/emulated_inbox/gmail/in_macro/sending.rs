//! The owner sending mail from Macro: durable send admission queues it, and
//! the scheduled delivery worker sends it through Gmail.

use super::{GmailInboxInMacro, RECIPIENT, owner};
use crate::pubsub::emulated_inbox::gmail::OWNER;
use crate::pubsub::test_support::fake_sqs;
use email::domain::models::{ContactInfo, CreateDraftInput};
use email::domain::send_attempt::{
    EmailSendService, SendAttemptId, SendAttemptStatus, SendSnapshot,
};
use models_email::service::pubsub::ScheduledPubsubMessage;
use uuid::Uuid;

/// A message the owner sent, or scheduled, from Macro.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub subject: String,
    attempt: SendAttemptId,
    message_id: Uuid,
}

impl GmailInboxInMacro {
    /// The owner sends `subject` to the recipient from Macro.
    pub async fn owner_sends(&self, subject: &str) -> Outgoing {
        self.owner_sends_at(subject, None).await
    }

    /// The owner schedules `subject` to the recipient for an hour from now.
    pub async fn owner_schedules(&self, subject: &str) -> Outgoing {
        self.owner_sends_at(
            subject,
            Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        )
        .await
    }

    /// Time passes until every scheduled send and every delivery retry is
    /// due, the scheduler hands them to the scheduled queue, and the
    /// workers run.
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
        self.email
            .email_send_status(owner(), self.link_id, outgoing.attempt)
            .await
            .expect("the send's status")
            .expect("Macro knows the send")
            .status
    }

    /// Whether Macro still keeps the sent message as a draft.
    pub async fn macro_keeps_a_draft(&self, outgoing: &Outgoing) -> bool {
        sqlx::query_scalar("SELECT is_draft FROM email_messages WHERE id = $1")
            .bind(outgoing.message_id)
            .fetch_optional(&self.db)
            .await
            .unwrap()
            .unwrap_or(false)
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

    async fn owner_sends_at(
        &self,
        subject: &str,
        send_time: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Outgoing {
        let attempt = SendAttemptId(Uuid::now_v7());
        let snapshot = SendSnapshot {
            message: CreateDraftInput {
                frozen_signature_html: None,
                draft_generation: 0,
                sender_email: None,
                db_id: Some(Uuid::now_v7()),
                provider_id: None,
                replying_to_id: None,
                provider_thread_id: None,
                thread_db_id: Some(Uuid::now_v7()),
                subject: subject.to_string(),
                to: vec![ContactInfo {
                    email: RECIPIENT.to_string(),
                    name: None,
                    photo_url: None,
                }],
                cc: vec![],
                bcc: vec![],
                body_text: Some("See you then.".to_string()),
                body_html: None,
                body_macro: None,
                headers_json: None,
                send_time,
                include_signature: Some(false),
                actor: None,
                draft_client_binding: None,
                thread_client_binding: None,
            },
            attachment_ids: vec![],
            forwarded_attachment_ids: vec![],
            restore_body_html: None,
            restore_body_text: None,
            restore_body_macro: None,
        };
        let admitted = self
            .email
            .send_email(owner(), self.link_id, attempt, snapshot)
            .await
            .expect("the send is admitted");
        Outgoing {
            subject: subject.to_string(),
            attempt,
            message_id: admitted.message_id.expect("an admitted message"),
        }
    }
}
