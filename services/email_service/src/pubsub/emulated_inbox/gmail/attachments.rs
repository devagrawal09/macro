//! Gmail mail with attachments, and what Macro shows of them.

use super::super::attachments::{self, Listed};
use super::{GmailInbox, Mail, Received};
use crate::pubsub::test_support::{self, GmailEndpoints, fake_sqs};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use email::domain::draft_attachments::AttachmentError;
use email::domain::mailbox::projection::MailboxAttachmentAccess;
use email::domain::mailbox::{MailboxError, MailboxKey};
use gmail_emulator::NewAttachment;

#[cfg(test)]
mod specs;

impl Mail {
    /// The mail carries a file.
    pub fn attaching(mut self, filename: &str, mime_type: &str, bytes: &[u8]) -> Mail {
        self.0.attachments.push(NewAttachment {
            filename: filename.to_string(),
            mime_type: Some(mime_type.to_string()),
            text: None,
            base64: Some(STANDARD.encode(bytes)),
        });
        self
    }
}

impl GmailInbox {
    /// The attachments Macro lists on `received`.
    pub async fn attachments_of(&self, received: &Received) -> Vec<Listed> {
        attachments::listed(&self.db, self.link_id, &received.id).await
    }

    /// The bytes Macro serves when the owner opens `attachment`.
    pub async fn opens(&self, attachment: &Listed) -> Result<Vec<u8>, AttachmentError> {
        let redis = redis::Client::open(test_support::redis_url()).expect("redis url");
        let gmail = test_support::gmail_api(
            self.db.clone(),
            &redis,
            GmailEndpoints {
                gmail: &self.gmail.gmail_url(),
                people: &self.gmail.people_url(),
                certs: &self.gmail.certs_url(),
                auth: &self.gmail.authentication_service_url(),
            },
            sqs_client::SQS::new(fake_sqs::client(&self.queues.sqs_url(), false)),
        )
        .await;
        attachments::served(&self.db, gmail, NoOutlook, attachment.id).await
    }
}

/// The Outlook side of attachment reads, which a Gmail inbox never reaches.
struct NoOutlook;

impl MailboxAttachmentAccess for NoOutlook {
    async fn download(
        &self,
        _mailbox: MailboxKey,
        _message_id: &str,
        _attachment_id: &str,
    ) -> Result<Vec<u8>, MailboxError> {
        Err(MailboxError::Persistence)
    }
}
