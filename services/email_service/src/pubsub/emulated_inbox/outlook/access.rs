//! The owner's Microsoft grant: revoked outside Macro, and given again by
//! reconnecting the inbox.

use super::{OWNER, OutlookInbox};
use crate::outbound::mailbox_init::{MicrosoftGrantSource, PgMailboxInitialization};
use authentication_service_client::AuthServiceClient;
use email::domain::mailbox::initialization::{InitializeMailbox, MailboxInitializationService};
use macro_user_id::user_id::MacroUserIdStr;
use uuid::Uuid;

#[cfg(test)]
mod specs;

/// The Microsoft client the reconnect consents to; the emulator accepts any.
const CLIENT_ID: &str = "macro";
const REDIRECT_URI: &str = "http://localhost/email/callback";

impl OutlookInbox {
    /// The owner removes Macro's access to their Microsoft account (or an
    /// admin revokes it): refreshing the grant answers `invalid_grant`.
    pub fn owner_revokes_access(&self) {
        self.outlook.revoke_grant(OWNER).expect("revoke the grant");
    }

    /// The owner reconnects the inbox: they consent again on Microsoft's
    /// sign-in page, and `/email/init` adopts the new grant for the inbox.
    pub async fn owner_reconnects(&self) {
        self.consents_on_microsofts_page().await;

        let attempt = Uuid::now_v7();
        let fusion_id = Uuid::now_v7();
        let auth = AuthServiceClient::new(
            "secret".to_string(),
            self.outlook.authentication_service_url(),
        );
        self.outlook
            .link_mailbox(&attempt.to_string(), OWNER)
            .expect("bind the attempt");
        self.started_a_connect_attempt(&auth, attempt, fusion_id)
            .await;

        let initialized = MailboxInitializationService::new(
            PgMailboxInitialization(self.db.clone()),
            MicrosoftGrantSource(auth),
        )
        .initialize(InitializeMailbox {
            attempt,
            actor: MacroUserIdStr::try_from(format!("macro|{OWNER}")).unwrap(),
            actor_fusion_id: fusion_id,
            force_share: false,
        })
        .await
        .expect("reconnect");
        assert_eq!(initialized, self.link_id, "reconnected another inbox");
    }

    /// Whether Macro asks the owner to reconnect the inbox.
    pub async fn asks_owner_to_reconnect(&self) -> bool {
        sqlx::query_scalar("SELECT needs_reauth FROM email_links WHERE id = $1")
            .bind(self.link_id)
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    async fn consents_on_microsofts_page(&self) {
        let request = [
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("response_type", "code"),
            ("scope", "offline_access Mail.ReadWrite Mail.Send"),
            ("state", "reconnect"),
            ("user", OWNER),
        ];
        let answered = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .post(format!(
                "{}/common/oauth2/v2.0/authorize",
                self.outlook.base_url()
            ))
            .form(&request)
            .send()
            .await
            .expect("sign-in page");
        assert!(
            answered.status().is_redirection(),
            "consent refused: {}",
            answered.status()
        );
    }

    /// What the connect flow leaves before `/email/init`: the pending OAuth
    /// attempt of the owner's account. Also records the Microsoft identity
    /// the inbox was first connected with, which [`OutlookInbox::connected`]
    /// leaves out and a reconnect checks.
    async fn started_a_connect_attempt(
        &self,
        auth: &AuthServiceClient,
        attempt: Uuid,
        fusion_id: Uuid,
    ) {
        let grant = auth
            .completed_microsoft_grant(attempt, fusion_id)
            .await
            .expect("the attempt's grant");
        sqlx::query(
            "UPDATE email_links SET provider_tenant_id = $2, provider_mailbox_id = $3 WHERE id = $1",
        )
        .bind(self.link_id)
        .bind(&grant.tenant_id)
        .bind(&grant.mailbox_id)
        .execute(&self.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO macro_user (id, username, email, stripe_customer_id) VALUES ($1, $2, $2, 'cus_test')",
        )
        .bind(fusion_id)
        .bind(OWNER)
        .execute(&self.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO in_progress_user_link (id, macro_user_id, email_provider, linked_email)
             VALUES ($1, $2, 'OUTLOOK', $3)",
        )
        .bind(attempt)
        .bind(fusion_id)
        .bind(OWNER)
        .execute(&self.db)
        .await
        .unwrap();
    }
}
