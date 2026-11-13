//! What the owner does in Macro reaches Outlook, and what they send arrives
//! once.

use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use outlook_emulator::{FaultError, Route};
use sqlx::PgPool;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_read_in_macro_is_read_in_outlook(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;

    inbox.owner_reads(&mail).await;
    inbox.changes_reach_outlook().await;

    assert!(inbox.in_outlook(&mail).is_read);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_archived_in_macro_is_in_the_outlook_archive(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;

    inbox.owner_archives(&mail).await;
    inbox.changes_reach_outlook().await;

    assert_eq!(inbox.in_outlook(&mail).folder, "Archive");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_flagged_in_macro_is_flagged_in_outlook(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;

    inbox.owner_flags(&mail).await;
    inbox.changes_reach_outlook().await;

    assert!(inbox.in_outlook(&mail).flagged);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_trashed_in_macro_is_in_outlooks_deleted_items(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;

    inbox.owner_trashes(&mail).await;
    inbox.changes_reach_outlook().await;

    assert_eq!(inbox.in_outlook(&mail).folder, "Deleted Items");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_archive_in_macro_survives_a_brief_graph_outage(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;
    inbox.graph_fails(Route::MessagesMove, FaultError::Internal, 3);

    inbox.owner_archives(&mail).await;
    inbox.changes_reach_outlook().await;
    for _ in 0..3 {
        inbox.time_passes().await;
    }
    inbox.syncs().await;

    assert_eq!(inbox.in_outlook(&mail).folder, "Archive");
    assert!(!inbox.macro_shows(&mail).await.in_inbox);
}

/// Macro reads the mailbox from Outlook again, here because Graph dropped
/// its sync state, after the owner archived the mail in Macro and before
/// the archive reached Outlook.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_archive_in_macro_is_kept_when_outlook_is_reread_before_it_lands(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    let mail = inbox.receives(RECIPIENT, "Q3 plan");
    inbox.syncs().await;

    inbox.owner_archives(&mail).await;
    inbox.graph_forgets_the_sync_state();
    inbox.syncs().await;
    let shown_before_outlook_has_it = inbox.macro_shows(&mail).await;
    inbox.changes_reach_outlook().await;
    inbox.syncs().await;

    assert!(
        !shown_before_outlook_has_it.in_inbox,
        "{shown_before_outlook_has_it:?}"
    );
    assert!(!inbox.macro_shows(&mail).await.in_inbox);
    assert_eq!(inbox.in_outlook(&mail).folder, "Archive");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_from_macro_arrives_once(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_outlook().await;
    inbox.time_passes().await;

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?}",
        inbox.send_status(&note).await
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.outlook_drafts(&note), 0);
    assert!(!inbox.macro_keeps_a_draft(&note).await);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_scheduled_in_macro_arrives_once_at_its_time(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;

    let note = inbox.owner_schedules("Friday").await;
    inbox.changes_reach_outlook().await;
    let early_copies = inbox.recipient_copies(&note);
    inbox.time_passes().await;
    inbox.time_passes().await;

    assert_eq!(early_copies, 0);
    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?}",
        inbox.send_status(&note).await
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.outlook_drafts(&note), 0);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_while_graph_fails_once_still_arrives_once(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    inbox.graph_fails(Route::MessagesSend, FaultError::Internal, 1);

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_outlook().await;
    for _ in 0..3 {
        inbox.time_passes().await;
    }

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?}",
        inbox.send_status(&note).await
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_when_graphs_answer_is_lost_arrives_once(pool: PgPool) {
    let inbox = OutlookInboxInMacro::connected(pool).await;
    inbox.graph_fails(Route::MessagesSend, FaultError::AnswerLost, 1);

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_outlook().await;
    for _ in 0..3 {
        inbox.time_passes().await;
    }

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?}",
        inbox.send_status(&note).await
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}
