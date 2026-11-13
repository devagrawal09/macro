//! Outlook mail with attachments, and what Macro shows of them.

use super::super::attachments::{self, Listed};
use super::{OWNER, OutlookInbox, Received};
use crate::pubsub::test_support::{self, GmailEndpoints, fake_sqs};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use email::domain::draft_attachments::AttachmentError;
use outlook_emulator::{NewAttachment, NewMessage};

#[cfg(test)]
mod specs;

/// A file on mail arriving in Outlook.
pub fn file(filename: &str, mime_type: &str, bytes: &[u8]) -> NewAttachment {
    NewAttachment {
        filename: filename.to_string(),
        mime_type: Some(mime_type.to_string()),
        text: None,
        base64: Some(STANDARD.encode(bytes)),
        content_id: None,
    }
}

/// An image the body shows in place, by its `Content-ID`.
pub fn inline_image(filename: &str, content_id: &str, bytes: &[u8]) -> NewAttachment {
    NewAttachment {
        content_id: Some(content_id.to_string()),
        ..file(filename, "image/png", bytes)
    }
}

impl OutlookInbox {
    /// Mail carrying `attachments` arrives in the Outlook inbox.
    pub fn receives_attached(&self, sender: &str, attachments: Vec<NewAttachment>) -> Received {
        let delivered = self
            .outlook
            .deliver(
                OWNER,
                &NewMessage {
                    from: Some(sender.to_string()),
                    subject: Some("Files".to_string()),
                    html: Some("<p>Attached.</p>".to_string()),
                    attachments,
                    ..NewMessage::default()
                },
            )
            .expect("delivery");
        Received { id: delivered.id }
    }

    /// The attachments Macro lists on `received`.
    pub async fn attachments_of(&self, received: &Received) -> Vec<Listed> {
        attachments::listed(&self.db, self.link_id, &received.id).await
    }

    /// The bytes Macro serves when the owner opens `attachment`.
    pub async fn opens(&self, attachment: &Listed) -> Result<Vec<u8>, AttachmentError> {
        // The Gmail side of attachment reads, which an Outlook inbox never
        // reaches; it points nowhere.
        let redis = redis::Client::open(test_support::redis_url()).expect("redis url");
        let nowhere = "http://127.0.0.1:1";
        let gmail = test_support::gmail_api(
            self.db.clone(),
            &redis,
            GmailEndpoints {
                gmail: nowhere,
                people: nowhere,
                certs: nowhere,
                auth: nowhere,
            },
            sqs_client::SQS::new(fake_sqs::client(nowhere, false)),
        )
        .await;
        attachments::served(&self.db, gmail, self.graph(), attachment.id).await
    }
}
