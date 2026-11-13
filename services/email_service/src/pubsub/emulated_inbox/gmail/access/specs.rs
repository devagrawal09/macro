//! What the owner sees after taking Macro's access away.

use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_owner_who_removed_macros_access_is_asked_to_reconnect(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    inbox.receives(Mail::from("lee@example.com"));
    inbox.syncs().await;
    inbox.owner_removes_macros_access();
    inbox.receives(Mail::from("lee@example.com"));

    inbox.syncs().await;

    assert!(inbox.asks_owner_to_reconnect().await);
    assert!(
        inbox.dead_letters().is_empty(),
        "{:?}",
        inbox.dead_letters()
    );
}
