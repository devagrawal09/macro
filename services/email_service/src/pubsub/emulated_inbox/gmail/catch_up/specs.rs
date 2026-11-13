//! Mail that arrived while Macro was away longer than Gmail keeps history.

use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_from_a_long_absence_arrives_once(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let before = inbox.receives(Mail::from("lee@example.com").subject("Before"));
    inbox.syncs().await;
    let missed = inbox.receives_while_away(Mail::from("kim@example.com").subject("Missed"));
    inbox.is_away_longer_than_gmail_keeps_history();
    let latest = inbox.receives(Mail::from("ray@example.com").subject("Latest"));

    inbox.syncs().await;

    for mail in [&before, &missed, &latest] {
        assert!(
            inbox.shows(mail).await.is_some(),
            "{mail:?} missing; dead letters: {:?}",
            inbox.dead_letters()
        );
    }
    assert_eq!(inbox.messages().await.len(), 3);
    assert!(
        inbox.dead_letters().is_empty(),
        "{:?}",
        inbox.dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_reply_from_a_long_absence_joins_its_thread(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let plan = inbox.receives(Mail::from("lee@example.com").subject("Q3 plan"));
    inbox.syncs().await;
    let reply = inbox.receives_while_away(Mail::from("lee@example.com").replying_to(&plan));
    inbox.is_away_longer_than_gmail_keeps_history();

    inbox.syncs().await;

    let shown = inbox
        .shows(&reply)
        .await
        .unwrap_or_else(|| panic!("reply missing; dead letters: {:?}", inbox.dead_letters()));
    assert_eq!(shown.provider_thread_id, Some(plan.thread_id.clone()));
    assert_eq!(inbox.messages().await.len(), 2);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_after_catching_up_syncs_as_usual(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    inbox.receives_while_away(Mail::from("kim@example.com"));
    inbox.is_away_longer_than_gmail_keeps_history();
    inbox.syncs().await;
    let next = inbox.receives(Mail::from("ray@example.com"));

    inbox.syncs().await;

    assert!(
        inbox.shows(&next).await.is_some(),
        "lost: {:?}",
        inbox.dead_letters()
    );
    assert_eq!(inbox.messages().await.len(), 2);
}
