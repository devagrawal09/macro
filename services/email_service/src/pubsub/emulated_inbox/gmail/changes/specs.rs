//! What the owner does in Macro reaches Gmail, and what they send arrives
//! once.

use super::*;
use crate::pubsub::emulated_inbox::gmail::Mail;
use email::domain::send_attempt::SendAttemptStatus;
use gmail_emulator::{FaultError, Route};
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use sqlx::PgPool;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_read_in_macro_is_read_in_gmail(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;

    inbox.owner_reads(&mail).await;
    inbox.changes_reach_gmail().await;

    assert!(
        !inbox.gmail_labels(&mail).contains("UNREAD"),
        "{:?}",
        inbox.all_dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_archived_in_macro_leaves_the_gmail_inbox(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;

    inbox.owner_archives(&mail).await;
    inbox.changes_reach_gmail().await;

    assert!(
        !inbox.gmail_labels(&mail).contains("INBOX"),
        "{:?}",
        inbox.all_dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_starred_in_macro_is_starred_in_gmail(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;

    inbox.owner_stars(&mail).await;
    inbox.changes_reach_gmail().await;

    assert!(
        inbox.gmail_labels(&mail).contains("STARRED"),
        "{:?}",
        inbox.all_dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_trashed_in_macro_is_in_the_gmail_trash(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;

    inbox.owner_trashes(&mail).await;
    inbox.changes_reach_gmail().await;

    assert!(
        inbox.gmail_labels(&mail).contains("TRASH"),
        "{:?}",
        inbox.all_dead_letters()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_archive_in_macro_survives_a_brief_gmail_outage(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;
    inbox.gmail_fails(Route::MessagesModify, FaultError::Internal, 3);

    inbox.owner_archives(&mail).await;
    inbox.changes_reach_gmail().await;
    inbox.syncs().await;

    assert!(
        !inbox.gmail_labels(&mail).contains("INBOX"),
        "{:?}",
        inbox.all_dead_letters()
    );
    assert!(!inbox.macro_labels(&mail).await.contains("INBOX"));
}

/// The owner reads the mail in Gmail, then archives it in Macro before
/// Gmail's notification of the read is synced.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_archive_in_macro_is_kept_when_an_earlier_gmail_change_syncs(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    let mail = inbox.receives(Mail::from(RECIPIENT));
    inbox.syncs().await;
    inbox.owner_reads_in_gmail(&mail);

    inbox.owner_archives(&mail).await;
    inbox.syncs().await;
    let shown_before_gmail_has_it = inbox.macro_labels(&mail).await;
    inbox.changes_reach_gmail().await;

    assert!(
        !shown_before_gmail_has_it.contains("INBOX"),
        "{shown_before_gmail_has_it:?}"
    );
    assert!(!inbox.macro_labels(&mail).await.contains("INBOX"));
    assert!(!inbox.gmail_labels(&mail).contains("INBOX"));
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_from_macro_arrives_once(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_gmail().await;

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?} {:?}",
        inbox.send_status(&note).await,
        inbox.all_dead_letters()
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.gmail_drafts(&note), 0);
    assert!(!inbox.macro_keeps_a_draft(&note).await);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_scheduled_in_macro_arrives_once_at_its_time(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;

    let note = inbox.owner_schedules("Friday").await;
    inbox.changes_reach_gmail().await;
    let early_copies = inbox.recipient_copies(&note);
    inbox.time_passes_and_the_scheduler_runs().await;

    assert_eq!(early_copies, 0);
    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?} {:?}",
        inbox.send_status(&note).await,
        inbox.all_dead_letters()
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.gmail_drafts(&note), 0);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_while_gmail_fails_once_still_arrives_once(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    inbox.gmail_fails(Route::MessagesSend, FaultError::Internal, 1);

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_gmail().await;
    inbox.time_passes_and_the_scheduler_runs().await;
    inbox.time_passes_and_the_scheduler_runs().await;

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?} {:?}",
        inbox.send_status(&note).await,
        inbox.all_dead_letters()
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_sent_when_gmails_answer_is_lost_arrives_once(pool: PgPool) {
    let mut inbox = GmailInboxInMacro::connected(pool).await;
    inbox.gmail_fails(Route::MessagesSend, FaultError::AnswerLost, 1);

    let note = inbox.owner_sends("Friday").await;
    inbox.changes_reach_gmail().await;
    inbox.time_passes_and_the_scheduler_runs().await;
    inbox.time_passes_and_the_scheduler_runs().await;

    assert_eq!(
        inbox.recipient_copies(&note),
        1,
        "{:?} {:?}",
        inbox.send_status(&note).await,
        inbox.all_dead_letters()
    );
    assert_eq!(inbox.sent_copies(&note), 1);
    assert_eq!(inbox.send_status(&note).await, SendAttemptStatus::Sent);
}
