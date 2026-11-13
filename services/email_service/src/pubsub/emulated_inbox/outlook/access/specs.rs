//! An Outlook inbox whose Microsoft access is revoked.

use super::super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_owner_who_revoked_outlook_access_is_asked_to_reconnect(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.receives("lee@example.com", "First");
    inbox.syncs().await;

    inbox.owner_revokes_access();
    inbox.receives("lee@example.com", "Second");
    inbox.syncs().await;
    inbox.syncs_after_backoff().await;

    assert!(inbox.asks_owner_to_reconnect().await);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_that_arrived_while_access_was_revoked_shows_up_after_reconnecting(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.receives("lee@example.com", "First");
    inbox.syncs().await;
    inbox.owner_revokes_access();
    let missed = inbox.receives("lee@example.com", "While revoked");
    inbox.syncs().await;
    inbox.syncs_after_backoff().await;

    inbox.owner_reconnects().await;
    let later = inbox.receives("lee@example.com", "After reconnecting");
    inbox.syncs().await;

    assert!(!inbox.asks_owner_to_reconnect().await);
    assert!(inbox.shows(&missed).await, "missed mail never arrived");
    assert!(inbox.shows(&later).await, "later mail never arrived");
}
