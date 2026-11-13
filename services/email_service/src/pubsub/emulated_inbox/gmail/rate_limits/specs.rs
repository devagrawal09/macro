//! Mail Gmail held back behind its rate limits.

use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_arrives_once_gmail_lifts_its_rate_limit(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::from("lee@example.com"));
    inbox.gmail_rate_limits(Route::MessagesGet, 5);

    inbox.syncs().await;

    assert!(
        inbox.shows(&mail).await.is_some(),
        "lost: {:?}",
        inbox.dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_arrives_when_gmail_rate_limits_reading_what_changed(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::from("lee@example.com"));
    inbox.gmail_rate_limits(Route::HistoryList, 5);

    inbox.syncs().await;

    assert!(
        inbox.shows(&mail).await.is_some(),
        "lost: {:?}",
        inbox.dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_arrives_once_the_owners_gmail_quota_recovers(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::from("lee@example.com"));
    inbox.owners_gmail_quota_runs_out(Route::MessagesGet, 5);

    inbox.syncs().await;

    assert!(
        inbox.shows(&mail).await.is_some(),
        "lost: {:?}",
        inbox.dead_letters()
    );
}
