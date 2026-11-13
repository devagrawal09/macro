//! The owner composing in Macro for an Outlook inbox, and the draft loop
//! (`mailbox_sync_worker::run_drafts`) writing those drafts to Outlook.

use super::{OWNER, OutlookInbox};
use crate::outbound::email_api::{MicrosoftCredentialsClient, ProviderMailboxGateway};
use email::domain::attachment_access::AttachmentReadRecord;
use email::domain::mailbox::MailboxError;
use email::domain::mailbox::credentials::MailboxCredentials;
use email::domain::mailbox::drafts::MailboxDraftService;
use email::domain::mailbox::drafts::content::{DraftContentBytes, DraftContentService};
use email::domain::models::CreateDraftInput;
use email::domain::ports::EmailService;
use email::outbound::EmailPgRepo;
use email::outbound::mailbox_pg::PgMailboxSync;
use email_api_client::OutlookApiClientRepository;
use email_api_client::domain::ports::AlwaysAllowRateLimiter;
use email_api_client::domain::service::mailbox::MailboxApiService;
use macro_service_urls::MicrosoftGraphUrl;
use macro_user_id::user_id::MacroUserIdStr;
use uuid::Uuid;

#[cfg(test)]
mod specs;

/// More passes than any spec needs; a draft loop that doesn't settle by
/// then is looping.
const MAX_DRAFT_PASSES: usize = 50;

/// One draft the owner writes in Macro's composer, under the client handle
/// the composer keeps for it across autosaves.
#[derive(Clone)]
pub struct Composed {
    input: CreateDraftInput,
}

impl OutlookInbox {
    /// The owner starts a draft in Macro's composer, from this inbox.
    pub fn composes(&self, subject: &str) -> Composed {
        Composed {
            input: CreateDraftInput {
                frozen_signature_html: None,
                draft_generation: 0,
                sender_email: None,
                db_id: Some(Uuid::now_v7()),
                provider_id: None,
                replying_to_id: None,
                provider_thread_id: None,
                thread_db_id: Some(Uuid::now_v7()),
                subject: subject.to_string(),
                to: vec![],
                cc: vec![],
                bcc: vec![],
                body_text: Some("Ignore this.".to_string()),
                body_html: None,
                body_macro: None,
                headers_json: None,
                send_time: None,
                include_signature: Some(false),
                actor: None,
                draft_client_binding: None,
                thread_client_binding: None,
            },
        }
    }

    /// The composer autosaves `draft`. A save Macro refuses is an answer
    /// the composer shows, not a test failure.
    pub async fn autosaves(&self, draft: &Composed) {
        self.email_service()
            .save_draft_for_user(self.owner(), Some(self.link_id), draft.input.clone())
            .await
            .ok();
    }

    /// The owner discards `draft` in the composer.
    pub async fn discards(&self, draft: &Composed) {
        self.email_service()
            .delete_draft_for_user(
                self.owner(),
                draft.input.db_id.expect("a client handle"),
                draft.input.draft_generation,
            )
            .await
            .expect("discard");
    }

    /// The draft loop runs until it has nothing left to write to Outlook,
    /// then the sync loops pick up what it wrote. The loop parks a draft
    /// for a few seconds between its steps; that time passes too.
    pub async fn draft_loop_runs(&self) {
        let drafts = self.draft_service();
        for _ in 0..MAX_DRAFT_PASSES {
            sqlx::query(
                "UPDATE email_mailbox_drafts SET available_at = now()
                 WHERE link_id = $1 AND lease_id IS NULL",
            )
            .bind(self.link_id)
            .execute(&self.db)
            .await
            .unwrap();
            // A failed step is released for a retry, as the worker loop
            // leaves it, so it counts as no work done in this pass.
            if !drafts.execute_once().await.unwrap_or(false) {
                self.syncs().await;
                return;
            }
        }
        panic!("the Outlook draft loop did not settle");
    }

    /// The subjects of the drafts Macro lists for the inbox.
    pub async fn drafts_in_macro(&self) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT m.subject FROM email_messages m
             JOIN email_message_mailbox_facts f ON f.id = m.id
             WHERE m.link_id = $1 AND m.is_draft AND f.is_present
             ORDER BY m.created_at",
        )
        .bind(self.link_id)
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    /// The subjects of the drafts the Outlook mailbox holds.
    pub fn drafts_in_outlook(&self) -> Vec<String> {
        self.outlook
            .messages(OWNER)
            .expect("the owner's mailbox")
            .into_iter()
            .filter(|message| message.is_draft)
            .map(|message| message.subject)
            .collect()
    }

    fn owner(&self) -> MacroUserIdStr<'static> {
        MacroUserIdStr::try_from(format!("macro|{OWNER}")).unwrap()
    }

    /// What the draft routes call, as `composition::api` wires it, minus
    /// sending, CRM and events, which these specs don't reach.
    fn email_service(&self) -> impl EmailService {
        email::domain::service::EmailServiceImpl::new(
            EmailPgRepo::new(self.db.clone()),
            frecency::domain::services::FrecencyQueryServiceImpl::new(
                frecency::outbound::postgres::FrecencyPgStorage::new(self.db.clone()),
            ),
            email::domain::ports::NoOpEnqueuer,
            crm::domain::service::NoOpCrmService,
            entity_access_management::domain::service::EntityAccessManagementServiceImpl::new(
                entity_access_management::outbound::PgRepository::new(self.db.clone()),
            ),
            0,
        )
    }

    /// The draft loop as `pubsub_workers` wires it, except that draft files
    /// have no storage: these specs attach none.
    fn draft_service(
        &self,
    ) -> MailboxDraftService<
        PgMailboxSync,
        GraphGateway,
        DraftContentService<
            EmailPgRepo,
            NoDraftFiles,
            entity_access::domain::service::EntityAccessServiceImpl<
                entity_access::outbound::PgAccessRepository,
            >,
        >,
    > {
        MailboxDraftService::new(
            PgMailboxSync::new(self.db.clone()),
            self.graph(),
            DraftContentService {
                repository: EmailPgRepo::new(self.db.clone()),
                bytes: NoDraftFiles,
                access: entity_access::domain::service::EntityAccessServiceImpl::new(
                    entity_access::outbound::PgAccessRepository::new(self.db.clone()),
                ),
            },
        )
        .with_writes_enabled(true)
    }
}

/// Macro's Outlook client, as the workers wire it.
pub type GraphGateway = ProviderMailboxGateway<
    OutlookApiClientRepository,
    MailboxCredentials<PgMailboxSync, MicrosoftCredentialsClient>,
    AlwaysAllowRateLimiter,
>;

impl OutlookInbox {
    /// Macro's Outlook client for this inbox, as the workers wire it.
    pub fn graph(&self) -> GraphGateway {
        ProviderMailboxGateway(MailboxApiService::new(
            OutlookApiClientRepository::for_graph_url(&MicrosoftGraphUrl::from(
                self.outlook.graph_url(),
            ))
            .expect("outlook client"),
            MailboxCredentials::new(
                PgMailboxSync::new(self.db.clone()),
                MicrosoftCredentialsClient(authentication_service_client::AuthServiceClient::new(
                    "secret".to_string(),
                    self.outlook.authentication_service_url(),
                )),
            ),
            AlwaysAllowRateLimiter,
        ))
    }
}

/// Draft file storage for drafts without files.
pub struct NoDraftFiles;

impl DraftContentBytes for NoDraftFiles {
    async fn uploaded(&self, _key: &str) -> Result<Vec<u8>, MailboxError> {
        Err(MailboxError::Persistence)
    }

    async fn provider(&self, _record: &AttachmentReadRecord) -> Result<Vec<u8>, MailboxError> {
        Err(MailboxError::Persistence)
    }
}
