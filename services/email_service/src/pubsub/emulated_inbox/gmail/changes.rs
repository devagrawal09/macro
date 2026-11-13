//! Changes the owner makes in Gmail itself, and where Macro files a message.

use super::*;
use gmail_emulator::SeedLabel;

#[cfg(test)]
mod specs;

/// Where Macro files a stored message, and its flags.
#[derive(Debug, sqlx::FromRow)]
pub struct Filed {
    /// The names of the message's labels. Gmail names its system labels by
    /// their ids (`INBOX`, `TRASH`, `STARRED`).
    pub labels: Vec<String>,
    pub read: bool,
    pub starred: bool,
    /// Whether the inbox lists the message's thread.
    pub thread_in_inbox: bool,
}

impl Filed {
    /// The message carries Gmail's INBOX label and its thread is listed in
    /// the inbox.
    pub fn in_inbox(&self) -> bool {
        self.has_label("INBOX") && self.thread_in_inbox
    }

    pub fn in_trash(&self) -> bool {
        self.has_label("TRASH")
    }

    pub fn has_label(&self, name: &str) -> bool {
        self.labels.iter().any(|label| label == name)
    }
}

impl GmailInbox {
    /// A connected inbox whose owner made `labels` in Gmail.
    pub async fn connected_with_labels(db: PgPool, labels: &[&str]) -> GmailInbox {
        let mut mailbox = SeedMailbox::new(OWNER);
        mailbox.labels = labels
            .iter()
            .map(|name| SeedLabel {
                id: Some(label_id(name)),
                name: name.to_string(),
            })
            .collect();
        GmailInbox::connected_to(db, mailbox).await
    }

    /// The owner archives `message` in Gmail.
    pub fn archives_in_gmail(&self, message: &Received) {
        self.relabels_in_gmail(message, &[], &["INBOX"]);
    }

    /// The owner stars `message` in Gmail.
    pub fn stars_in_gmail(&self, message: &Received) {
        self.relabels_in_gmail(message, &["STARRED"], &[]);
    }

    /// The owner opens `message` in Gmail.
    pub fn reads_in_gmail(&self, message: &Received) {
        self.relabels_in_gmail(message, &[], &["UNREAD"]);
    }

    /// The owner marks `message` unread in Gmail.
    pub fn marks_unread_in_gmail(&self, message: &Received) {
        self.relabels_in_gmail(message, &["UNREAD"], &[]);
    }

    /// The owner moves `message` to Gmail's trash, which takes it out of
    /// the inbox.
    pub fn trashes_in_gmail(&self, message: &Received) {
        self.relabels_in_gmail(message, &["TRASH"], &["INBOX"]);
    }

    /// The owner deletes `message` forever in Gmail.
    pub fn deletes_in_gmail(&self, message: &Received) {
        self.gmail
            .delete_message(OWNER, &message.id)
            .expect("delete in Gmail");
    }

    /// The owner puts their label `name` on `message` in Gmail.
    pub fn labels_in_gmail(&self, message: &Received, name: &str) {
        self.gmail
            .modify_labels(OWNER, &message.id, &[label_id(name)], &[])
            .expect("label in Gmail");
    }

    /// Where Macro files `message`, if it stored it.
    pub async fn filed(&self, message: &Received) -> Option<Filed> {
        sqlx::query_as(
            "SELECT
                 COALESCE(array_agg(l.name ORDER BY l.name) FILTER (WHERE l.id IS NOT NULL), '{}') AS labels,
                 m.is_read AS read,
                 m.is_starred AS starred,
                 t.inbox_visible AS thread_in_inbox
             FROM email_messages m
             JOIN email_threads t ON t.id = m.thread_id
             LEFT JOIN email_message_labels ml ON ml.message_id = m.id
             LEFT JOIN email_labels l ON l.id = ml.label_id
             WHERE m.link_id = $1 AND m.provider_id = $2
             GROUP BY m.id, t.id",
        )
        .bind(self.link_id)
        .bind(&message.id)
        .fetch_optional(&self.db)
        .await
        .unwrap()
    }

    fn relabels_in_gmail(&self, message: &Received, add: &[&str], remove: &[&str]) {
        let owned = |labels: &[&str]| -> Vec<String> {
            labels.iter().map(|label| label.to_string()).collect()
        };
        self.gmail
            .modify_labels(OWNER, &message.id, &owned(add), &owned(remove))
            .expect("relabel in Gmail");
    }
}

/// The Gmail id of the owner's label `name`.
fn label_id(name: &str) -> String {
    format!("Label_{name}")
}
