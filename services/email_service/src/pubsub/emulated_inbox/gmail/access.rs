//! The owner taking Macro's access away in their Google account.

use super::*;

#[cfg(test)]
mod specs;

impl GmailInbox {
    /// The owner removes Macro from the apps with access to their Google
    /// account: its tokens stop working and it can't get new ones.
    pub fn owner_removes_macros_access(&self) {
        self.gmail.revoke_grant(OWNER).expect("revoke the grant");
    }
}
