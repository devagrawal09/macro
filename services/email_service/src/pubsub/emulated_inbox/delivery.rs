//! The owner's side in Macro, for either provider: the email domain service
//! the HTTP and GraphQL routes call, durable send admission, and what
//! sending needs besides the provider (the draft content reader the
//! scheduled delivery worker and the Outlook draft loop share, and read
//! receipts). No spec sends attachments, so S3 points at the stand-in
//! services.

#![expect(
    clippy::disallowed_methods,
    reason = "harness SQL reads and moves test state; it stays out of the offline query cache"
)]

use crate::outbound::email_api::{GmailApi, MicrosoftCredentialsClient, ProviderMailboxGateway};
use email::domain::mailbox::credentials::MailboxCredentials;
use email::domain::models::{ContactInfo, CreateDraftInput};
use email::domain::send_attempt::{
    EmailSendService, SendAttemptId, SendAttemptStatus, SendSnapshot,
};
use email::domain::service::EmailServiceImpl;
use email::outbound::EmailPgRepo;
use email::outbound::mailbox_pg::PgMailboxSync;
use email_api_client::OutlookApiClientRepository;
use email_api_client::domain::ports::AlwaysAllowRateLimiter;
use email_api_client::domain::service::mailbox::MailboxApiService;
use frecency::domain::services::FrecencyQueryServiceImpl;
use frecency::outbound::postgres::FrecencyPgStorage;
use macro_user_id::user_id::MacroUserIdStr;
use std::sync::Arc;
use uuid::Uuid;

const BUCKET: &str = "attachments";

/// The email domain service, as the API composes it.
pub type EmailSvc = EmailServiceImpl<
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

/// A message the owner sent, or scheduled, from Macro.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub subject: String,
    attempt: SendAttemptId,
    message_id: Uuid,
}

/// The email domain service, queueing through `sqs_client`, with no undo
/// window on sends.
pub fn email_service(db: &sqlx::PgPool, sqs_client: sqs_client::SQS) -> EmailSvc {
    EmailServiceImpl::new(
        EmailPgRepo::new(db.clone()),
        FrecencyQueryServiceImpl::new(FrecencyPgStorage::new(db.clone())),
        sqs_client,
        crm::domain::service::CrmServiceImpl::new(
            crm::outbound::companies_repo::CompaniesRepositoryImpl::new(db.clone()),
            crm::outbound::no_op_resolver::NoOpCompanyMetadataResolver,
        ),
        entity_access_management::domain::service::EntityAccessManagementServiceImpl::new(
            entity_access_management::outbound::PgRepository::new(db.clone()),
        ),
        0,
    )
}

pub fn macro_user(email: &str) -> MacroUserIdStr<'static> {
    MacroUserIdStr::try_from_email(email).unwrap()
}

/// `owner` sends `subject` to `recipient` from the inbox `link_id`, at
/// `send_time` or at once, as the GraphQL send mutation admits it.
pub async fn admit_send(
    email: &EmailSvc,
    owner: &str,
    link_id: Uuid,
    recipient: &str,
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
                email: recipient.to_string(),
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
    let admitted = email
        .send_email(macro_user(owner), link_id, attempt, snapshot)
        .await
        .expect("the send is admitted");
    Outgoing {
        subject: subject.to_string(),
        attempt,
        message_id: admitted.message_id.expect("an admitted message"),
    }
}

/// What Macro tells `owner` about the send.
pub async fn send_status(
    email: &EmailSvc,
    owner: &str,
    link_id: Uuid,
    outgoing: &Outgoing,
) -> SendAttemptStatus {
    email
        .email_send_status(macro_user(owner), link_id, outgoing.attempt)
        .await
        .expect("the send's status")
        .expect("Macro knows the send")
        .status
}

/// Whether Macro still keeps the sent message as a draft.
pub async fn keeps_a_draft(db: &sqlx::PgPool, outgoing: &Outgoing) -> bool {
    sqlx::query_scalar("SELECT is_draft FROM email_messages WHERE id = $1")
        .bind(outgoing.message_id)
        .fetch_optional(db)
        .await
        .unwrap()
        .unwrap_or(false)
}

/// The production Outlook mailbox API, and the gateway the loops call it
/// through.
pub type OutlookApi = MailboxApiService<
    OutlookApiClientRepository,
    MailboxCredentials<PgMailboxSync, MicrosoftCredentialsClient>,
    AlwaysAllowRateLimiter,
>;
pub type OutlookGateway = ProviderMailboxGateway<
    OutlookApiClientRepository,
    MailboxCredentials<PgMailboxSync, MicrosoftCredentialsClient>,
    AlwaysAllowRateLimiter,
>;

pub type DraftContent = Arc<
    email::domain::mailbox::drafts::content::DraftContentService<
        EmailPgRepo,
        crate::outbound::mailbox_drafts::MailboxDraftContent<
            crate::outbound::attachment_access::ProviderAttachmentBytes<OutlookGateway>,
        >,
        entity_access::domain::service::EntityAccessServiceImpl<
            entity_access::outbound::PgAccessRepository,
        >,
    >,
>;

/// The Outlook mailbox API for Graph at `graph_url`, with tokens from the
/// authentication service at `auth_url`.
pub fn outlook_api(db: &sqlx::PgPool, graph_url: &str, auth_url: &str) -> OutlookApi {
    MailboxApiService::new(
        OutlookApiClientRepository::for_graph_url(&macro_service_urls::MicrosoftGraphUrl::from(
            graph_url.to_string(),
        ))
        .expect("outlook client"),
        MailboxCredentials::new(
            PgMailboxSync::new(db.clone()),
            MicrosoftCredentialsClient(authentication_service_client::AuthServiceClient::new(
                "secret".to_string(),
                auth_url.to_string(),
            )),
        ),
        AlwaysAllowRateLimiter,
    )
}

/// The draft content reader, reading provider attachments through `gmail`
/// and `outlook`.
pub fn draft_content(
    db: &sqlx::PgPool,
    services_url: &str,
    gmail: GmailApi,
    outlook: OutlookApi,
) -> DraftContent {
    Arc::new(
        email::domain::mailbox::drafts::content::DraftContentService {
            repository: EmailPgRepo::new(db.clone()),
            bytes: crate::outbound::mailbox_drafts::MailboxDraftContent {
                provider: crate::outbound::attachment_access::ProviderAttachmentBytes {
                    s3: s3(services_url),
                    bucket: BUCKET.to_string(),
                    gmail,
                    outlook: ProviderMailboxGateway(outlook),
                },
                s3: s3(services_url),
                bucket: BUCKET.to_string(),
            },
            access: entity_access::domain::service::EntityAccessServiceImpl::new(
                entity_access::outbound::PgAccessRepository::new(db.clone()),
            ),
        },
    )
}

pub fn read_receipts(db: &sqlx::PgPool, services_url: &str) -> crate::composition::ReadReceipts {
    email::domain::read_receipts::ReadReceiptService::new(
        email::outbound::read_receipts_pg::ReadReceiptPgRepository(db.clone()),
        email::domain::read_receipts::PixelUrlPrefix::new(services_url),
    )
}

pub fn s3(services_url: &str) -> s3_client::S3 {
    s3_client::S3::new(aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                "test", "test", None, None, "harness",
            ))
            .endpoint_url(services_url)
            .build(),
    ))
}

pub fn bucket() -> String {
    BUCKET.to_string()
}
