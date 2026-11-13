//! Gmail mail with attachments.

use super::super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

const PDF: &[u8] = b"%PDF-1.4\n1 0 obj << >> endobj\ntrailer << >>\n%%EOF\n";

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_attachment_shows_with_its_name_type_and_size_and_opens(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::from("lee@example.com").attaching(
        "Q3 plan.pdf",
        "application/pdf",
        PDF,
    ));

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
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::from("lee@example.com").attaching(
        "Überblick – Q3 年度.pdf",
        "application/pdf",
        PDF,
    ));

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
    let mut inbox = GmailInbox::connected(pool).await;
    let name = format!("{}.pdf", "Überblick ".repeat(60));
    let mail =
        inbox.receives(Mail::from("lee@example.com").attaching(&name, "application/pdf", PDF));

    inbox.syncs().await;

    assert!(
        inbox.shows(&mail).await.is_some(),
        "lost: {:?}",
        inbox.dead_letters()
    );
    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), PDF);
}

/// Without its extension the owner's download opens in no application.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_very_long_attachment_name_keeps_its_extension(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let name = format!("{}.pdf", "Überblick ".repeat(60));
    let mail =
        inbox.receives(Mail::from("lee@example.com").attaching(&name, "application/pdf", PDF));

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    let shown = listed[0].filename.clone().unwrap_or_default();
    assert!(shown.ends_with(".pdf"), "shown as {shown:?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_inline_image_shows_with_its_content_id_and_opens(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::raw(
        "From: lee@example.com\r\nTo: owner@example.com\r\nSubject: Logo\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Our logo: <img src=\"cid:logo@example.com\"></p>\r\n--b1\r\nContent-Type: image/png; name=\"logo.png\"\r\nContent-Disposition: inline; filename=\"logo.png\"\r\nContent-ID: <logo@example.com>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--b1--\r\n",
    ));

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
    assert_eq!(inbox.opens(&listed[0]).await.unwrap(), b"\x89PNG\r\n\x1a\n");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_attached_email_shows_as_an_attachment_and_opens(pool: PgPool) {
    let mut inbox = GmailInbox::connected(pool).await;
    let mail = inbox.receives(Mail::raw(
        "From: lee@example.com\r\nTo: owner@example.com\r\nSubject: Fwd: Original\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b2\"\r\n\r\n--b2\r\nContent-Type: text/plain\r\n\r\nSee the attached mail.\r\n--b2\r\nContent-Type: message/rfc822; name=\"Original.eml\"\r\nContent-Disposition: attachment; filename=\"Original.eml\"\r\n\r\nFrom: kim@example.com\r\nTo: lee@example.com\r\nSubject: Original\r\n\r\nHello Lee.\r\n--b2--\r\n",
    ));

    inbox.syncs().await;

    let listed = inbox.attachments_of(&mail).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].filename.as_deref(), Some("Original.eml"));
    assert_eq!(listed[0].mime_type.as_deref(), Some("message/rfc822"));
    let opened = String::from_utf8(inbox.opens(&listed[0]).await.unwrap()).unwrap();
    assert!(opened.contains("Subject: Original"), "{opened:?}");
}
