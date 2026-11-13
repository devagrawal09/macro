//! Outlook mail with attachments.

use super::super::*;
use super::{file, inline_image};
use macro_db_migrator::MACRO_DB_MIGRATIONS;

const PDF: &[u8] = b"%PDF-1.4\n1 0 obj << >> endobj\ntrailer << >>\n%%EOF\n";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_attachment_shows_with_its_name_type_and_size_and_opens(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives_attached(
        "lee@example.com",
        vec![file("Q3 plan.pdf", "application/pdf", PDF)],
    );

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].filename.as_deref(), Some("Q3 plan.pdf"));
    assert_eq!(listed[0].mime_type.as_deref(), Some("application/pdf"));
    assert_eq!(listed[0].size_bytes, Some(PDF.len() as i64));
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), PDF);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_attachment_with_a_non_ascii_name_keeps_its_name(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives_attached(
        "lee@example.com",
        vec![file("Überblick – Q3 年度.pdf", "application/pdf", PDF)],
    );

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(
        listed
            .iter()
            .map(|a| a.filename.as_deref())
            .collect::<Vec<_>>(),
        [Some("Überblick – Q3 年度.pdf")]
    );
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), PDF);
}

/// Longer than the 512 characters Macro's attachment name column holds.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn mail_with_a_very_long_attachment_name_still_arrives_with_it(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let name = format!("{}.pdf", "Überblick ".repeat(60));
    let mail =
        inbox.receives_attached("lee@example.com", vec![file(&name, "application/pdf", PDF)]);

    inbox.syncs().await;

    assert!(inbox.shows(&mail).await);
    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), PDF);
}

/// Without its extension the owner's download opens in no application.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_very_long_attachment_name_keeps_its_extension(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let name = format!("{}.pdf", "Überblick ".repeat(60));
    let mail =
        inbox.receives_attached("lee@example.com", vec![file(&name, "application/pdf", PDF)]);

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    let shown = listed[0].filename.clone().unwrap_or_default();
    assert!(shown.ends_with(".pdf"), "shown as {shown:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_inline_image_shows_with_its_content_id_and_opens(pool: PgPool) {
    let inbox = OutlookInbox::connected(pool).await;
    let mail = inbox.receives_attached(
        "lee@example.com",
        vec![inline_image("logo.png", "logo@example.com", PNG)],
    );

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].filename.as_deref(), Some("logo.png"));
    assert_eq!(listed[0].mime_type.as_deref(), Some("image/png"));
    assert_eq!(
        listed[0]
            .content_id
            .as_deref()
            .map(|id| id.trim_matches(['<', '>'])),
        Some("logo@example.com")
    );
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), PNG);
}
