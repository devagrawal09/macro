//! Drafts the owner writes in Macro for an Outlook inbox.

use super::super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_draft_saved_in_macro_reaches_outlook(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.syncs().await;
    let draft = inbox.composes("Never mind");

    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;

    assert_eq!(inbox.drafts_in_outlook(), ["Never mind"]);
    assert_eq!(inbox.drafts_in_macro().await, ["Never mind"]);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_discarded_outlook_draft_stays_discarded_when_a_late_autosave_arrives(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.syncs().await;
    let draft = inbox.composes("Never mind");
    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;
    inbox.discards(&draft).await;
    inbox.draft_loop_runs().await;

    // The composer's last autosave left before the discard and arrives
    // after it, as a retried request does on a slow connection.
    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;

    assert_eq!(inbox.drafts_in_macro().await, Vec::<String>::new());
    assert_eq!(inbox.drafts_in_outlook(), Vec::<String>::new());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_draft_discarded_before_it_reached_outlook_stays_discarded(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.syncs().await;
    let draft = inbox.composes("Never mind");
    inbox.autosaves(&draft).await;
    inbox.discards(&draft).await;

    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;

    assert_eq!(inbox.drafts_in_macro().await, Vec::<String>::new());
    assert_eq!(inbox.drafts_in_outlook(), Vec::<String>::new());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_late_autosave_before_outlook_deleted_the_draft_does_not_bring_it_back(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    inbox.syncs().await;
    let draft = inbox.composes("Never mind");
    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;
    inbox.discards(&draft).await;

    inbox.autosaves(&draft).await;
    inbox.draft_loop_runs().await;

    assert_eq!(inbox.drafts_in_macro().await, Vec::<String>::new());
    assert_eq!(inbox.drafts_in_outlook(), Vec::<String>::new());
}
