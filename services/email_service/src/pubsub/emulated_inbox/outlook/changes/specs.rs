//! Changes the owner makes in Outlook reach Macro.

use super::super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_read_in_outlook_shows_as_read(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;

    inbox.owner_reads_in_outlook(&mail);
    inbox.syncs().await;

    assert!(inbox.shown(&mail).await.expect("stored").is_read);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_flagged_in_outlook_shows_as_flagged(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;

    inbox.owner_flags_in_outlook(&mail);
    inbox.syncs().await;

    assert!(inbox.shown(&mail).await.expect("stored").is_flagged);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_archived_in_outlook_leaves_the_inbox(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;

    inbox.owner_moves_in_outlook(&mail, "archive");
    inbox.syncs().await;

    let shown = inbox.shown(&mail).await.expect("stored");
    assert!(!shown.in_inbox && !shown.thread_in_inbox, "{shown:?}");
    assert!(shown.is_present, "{shown:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_deleted_in_outlook_moves_to_trash(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;

    inbox.owner_moves_in_outlook(&mail, "deleteditems");
    inbox.syncs().await;

    let shown = inbox.shown(&mail).await.expect("stored");
    assert!(shown.in_trash && !shown.thread_in_inbox, "{shown:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_deleted_for_good_in_outlook_disappears(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;

    inbox.owner_deletes_for_good_in_outlook(&mail);
    // Macro looks for a vanished message a few times before it believes it
    // gone, since Graph also reports a message moving between folders that
    // way.
    inbox.syncs().await;
    for _ in 0..3 {
        inbox.syncs_after_backoff().await;
    }

    let shown = inbox.shown(&mail).await;
    assert!(
        shown
            .as_ref()
            .is_none_or(|shown| !shown.is_present && !shown.thread_in_inbox),
        "{shown:?}"
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_moved_back_to_the_inbox_in_outlook_returns(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives("lee@example.com", "Q3 plan");
    inbox.syncs().await;
    inbox.owner_moves_in_outlook(&mail, "archive");
    inbox.syncs().await;

    inbox.owner_moves_in_outlook(&mail, "inbox");
    inbox.syncs().await;

    let shown = inbox.shown(&mail).await.expect("stored");
    assert!(shown.in_inbox && shown.thread_in_inbox, "{shown:?}");
}
