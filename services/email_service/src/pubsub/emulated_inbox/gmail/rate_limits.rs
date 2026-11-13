//! Gmail turning requests away for a while, and time passing until it lets
//! them through again.
//!
//! The request gate pauses a mailbox in Redis after a Gmail 429, by the real
//! clock. The harness delivers without waiting, so it ends that pause itself
//! where SQS would have waited at least as long: before a redelivery (the
//! queues' visibility timeout, 60 s, outlasts the gate's longest pause) and
//! before a message sent with a delay.

use super::*;

#[cfg(test)]
mod specs;

impl GmailInbox {
    /// Gmail answers the next `times` calls to `route` with 429
    /// `rateLimitExceeded`.
    pub fn gmail_rate_limits(&self, route: Route, times: u32) {
        self.gmail_fails(route, FaultError::RateLimit, times);
    }

    /// Gmail answers the next `times` calls to `route` with 403
    /// `userRateLimitExceeded`, the owner's per-user quota in its 403 form.
    pub fn owners_gmail_quota_runs_out(&self, route: Route, times: u32) {
        self.gmail_fails(route, FaultError::QuotaExceeded, times);
    }

    /// Ends the request gate's pause of the mailbox, as the time it lasts
    /// passing would.
    pub(super) async fn gmail_pause_passes(&self) {
        let scope = format!("gmail:{{{}}}", self.link_id);
        let mut redis = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .expect("redis");
        let _: () = redis::cmd("DEL")
            .arg(format!("{scope}:cooldown"))
            .arg(format!("{scope}:backfill_cooldown"))
            .query_async(&mut redis)
            .await
            .expect("end the request gate's pause");
    }
}
