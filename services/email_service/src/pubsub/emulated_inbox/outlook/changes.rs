//! Changes the owner makes to their mail in Outlook itself, and how Macro
//! shows the message after it syncs.

use super::{OWNER, OutlookInbox, Received};
use outlook_emulator::MessageChange;

#[cfg(test)]
mod specs;

/// How Macro shows a message it stored.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Shown {
    pub is_read: bool,
    pub is_flagged: bool,
    pub in_inbox: bool,
    pub in_trash: bool,
    /// Whether the message is still in the mailbox at all. A message gone
    /// from Outlook keeps its row, marked missing.
    pub is_present: bool,
    /// Whether its thread is listed in the Macro inbox.
    pub thread_in_inbox: bool,
}

impl OutlookInbox {
    /// The owner opens `received` in Outlook.
    pub fn owner_reads_in_outlook(&self, received: &Received) {
        self.owner_changes_in_outlook(
            received,
            MessageChange {
                read: Some(true),
                ..MessageChange::default()
            },
        );
    }

    /// The owner flags `received` in Outlook.
    pub fn owner_flags_in_outlook(&self, received: &Received) {
        self.owner_changes_in_outlook(
            received,
            MessageChange {
                flagged: Some(true),
                ..MessageChange::default()
            },
        );
    }

    /// The owner moves `received` to `folder` (a well-known name such as
    /// `archive` or `deleteditems`, or a display name) in Outlook.
    pub fn owner_moves_in_outlook(&self, received: &Received, folder: &str) {
        self.owner_changes_in_outlook(
            received,
            MessageChange {
                folder: Some(folder.to_string()),
                ..MessageChange::default()
            },
        );
    }

    /// The owner deletes `received` for good in Outlook (Shift+Delete, or
    /// emptying Deleted Items).
    pub fn owner_deletes_for_good_in_outlook(&self, received: &Received) {
        self.outlook
            .delete_message(OWNER, &received.id)
            .expect("delete in Outlook");
    }

    fn owner_changes_in_outlook(&self, received: &Received, change: MessageChange) {
        self.outlook
            .update_message(OWNER, &received.id, &change)
            .expect("change in Outlook");
    }

    /// How Macro shows `received`, if it stored it.
    pub async fn shown(&self, received: &Received) -> Option<Shown> {
        sqlx::query_as(
            "SELECT m.is_read, m.is_starred AS is_flagged,
                COALESCE(f.in_inbox, false) AS in_inbox, COALESCE(f.in_trash, false) AS in_trash,
                f.is_present, t.inbox_visible AS thread_in_inbox
             FROM email_messages m
             JOIN email_message_mailbox_facts f ON f.id = m.id
             JOIN email_threads t ON t.id = m.thread_id
             WHERE m.link_id = $1 AND m.provider_id = $2",
        )
        .bind(self.link_id)
        .bind(&received.id)
        .fetch_optional(&self.db)
        .await
        .unwrap()
    }
}
