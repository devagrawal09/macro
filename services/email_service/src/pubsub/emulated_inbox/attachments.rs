//! What Macro shows of a message's attachments, for either provider: the
//! attachment rows the message lists, and the bytes the attachment routes
//! (`AttachmentReadService`) serve for one, read through the production
//! record query and provider download.

use crate::outbound::attachment_access::ProviderAttachmentBytes;
use crate::outbound::email_api::GmailApi;
use email::domain::attachment_access::{AttachmentBytes, AttachmentReadRepository};
use email::domain::draft_attachments::AttachmentError;
use email::domain::mailbox::projection::MailboxAttachmentAccess;
use email::outbound::EmailPgRepo;
use sqlx::PgPool;
use uuid::Uuid;

/// An attachment as Macro lists it on its message.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Listed {
    pub id: Uuid,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub content_id: Option<String>,
}

/// The attachments Macro lists on the message with `provider_id`.
pub async fn listed(db: &PgPool, link_id: Uuid, provider_id: &str) -> Vec<Listed> {
    sqlx::query_as(
        "SELECT a.id, a.filename, a.mime_type, a.size_bytes, a.content_id
         FROM email_attachments a JOIN email_messages m ON m.id = a.message_id
         WHERE m.link_id = $1 AND m.provider_id = $2
         ORDER BY a.filename",
    )
    .bind(link_id)
    .bind(provider_id)
    .fetch_all(db)
    .await
    .unwrap()
}

/// The bytes Macro serves for the attachment `id`.
pub async fn served<A: MailboxAttachmentAccess>(
    db: &PgPool,
    gmail: GmailApi,
    outlook: A,
    id: Uuid,
) -> Result<Vec<u8>, AttachmentError> {
    let record = EmailPgRepo::new(db.clone())
        .attachment_read_record(id)
        .await?
        .ok_or(AttachmentError::NotFound)?;
    ProviderAttachmentBytes {
        s3: unused_s3(),
        bucket: "attachments".to_string(),
        gmail,
        outlook,
    }
    .bytes(&record)
    .await
}

/// Storage for attachments Macro holds itself (Outlook draft uploads);
/// mail that arrived is read from the provider, so nothing calls it.
fn unused_s3() -> s3_client::S3 {
    s3_client::S3::new(aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                "test", "test", None, None, "harness",
            ))
            .endpoint_url("http://127.0.0.1:1")
            .build(),
    ))
}
