//! What the owner does to mail in Gmail itself shows in Macro.

use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

/// An inbox that has stored one message from Lee.
async fn with_mail_from_lee(pool: PgPool) -> (GmailInbox, Received) {
    let mut inbox = GmailInbox::connected_with_labels(pool, &["Receipts"]).await;
    let mail = inbox.receives(Mail::from("lee@example.com"));
    inbox.syncs().await;
    (inbox, mail)
}

async fn filed(inbox: &GmailInbox, mail: &Received) -> Filed {
    inbox
        .filed(mail)
        .await
        .unwrap_or_else(|| panic!("not stored; dead letters: {:?}", inbox.dead_letters()))
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_archived_in_gmail_leaves_the_inbox(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    assert!(filed(&inbox, &mail).await.in_inbox());
    inbox.archives_in_gmail(&mail);

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(!filed.in_inbox(), "{filed:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_starred_in_gmail_is_starred(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.stars_in_gmail(&mail);

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(filed.starred, "{filed:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_read_in_gmail_is_read(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.reads_in_gmail(&mail);

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(filed.read, "{filed:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_marked_unread_in_gmail_is_unread_again(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.reads_in_gmail(&mail);
    inbox.syncs().await;
    inbox.marks_unread_in_gmail(&mail);

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(!filed.read, "{filed:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_trashed_in_gmail_moves_to_the_trash(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.trashes_in_gmail(&mail);

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(filed.in_trash() && !filed.in_inbox(), "{filed:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_deleted_forever_in_gmail_is_gone(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.deletes_in_gmail(&mail);

    inbox.syncs().await;

    assert!(inbox.shows(&mail).await.is_none());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_label_put_on_in_gmail_shows_on_the_mail(pool: PgPool) {
    let (mut inbox, mail) = with_mail_from_lee(pool).await;
    inbox.labels_in_gmail(&mail, "Receipts");

    inbox.syncs().await;

    let filed = filed(&inbox, &mail).await;
    assert!(filed.has_label("Receipts"), "{filed:?}");
}
