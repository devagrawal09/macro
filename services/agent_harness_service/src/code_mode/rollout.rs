//! A per-user rollout read from PostHog: document programs and the
//! marketplace of pages published from them.
use agent_code_mode::domain::DocumentProgramGate;
use async_trait::async_trait;
use feature_flags::FeatureFlags;
use macro_user_id::user_id::MacroUserIdStr;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use tokio::time::Instant;

/// How long a user's rollout answer is reused before PostHog is asked again.
const ROLLOUT_TTL: Duration = Duration::from_secs(60);
/// Users whose answers are kept; past it the least recent is asked again.
const CACHED_USERS: NonZeroUsize = NonZeroUsize::MIN.saturating_add(4095);

/// A feature that rolls out per user through one PostHog flag. Every request
/// asks, so a user's answer is kept for [`ROLLOUT_TTL`]; when PostHog fails,
/// the last answer stands, and a user with none is refused. A local stack
/// admits everyone, matching the web flags' dev default: its PostHog key is
/// often a placeholder that PostHog refuses. Without a flag provider (no
/// PostHog key) the feature is off.
pub struct FlaggedRollout<F> {
    flags: Option<F>,
    flag: &'static str,
    local: bool,
    answers: Mutex<lru::LruCache<MacroUserIdStr<'static>, Answer>>,
}

#[derive(Clone, Copy)]
struct Answer {
    enabled: bool,
    read_at: Instant,
}

impl<F> FlaggedRollout<F> {
    /// The rollout of `flag`. `local` admits everyone, for a local stack.
    pub fn new(flags: Option<F>, flag: &'static str, local: bool) -> Self {
        Self {
            flags,
            flag,
            local,
            answers: Mutex::new(lru::LruCache::new(CACHED_USERS)),
        }
    }
}

impl<F: FeatureFlags + 'static> FlaggedRollout<F> {
    /// Whether the rollout admits `user`.
    #[tracing::instrument(
        name = "feature_rollout",
        skip_all,
        fields(
            flag = self.flag,
            cache = tracing::field::Empty,
            allowed = tracing::field::Empty,
        )
    )]
    pub async fn allows(&self, user: &MacroUserIdStr<'static>) -> bool {
        if self.local {
            return true;
        }
        let Some(flags) = &self.flags else {
            return false;
        };
        let span = tracing::Span::current();
        let known = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(user)
            .copied();
        if let Some(answer) = known
            && answer.read_at.elapsed() < ROLLOUT_TTL
        {
            span.record("cache", "hit");
            span.record("allowed", answer.enabled);
            return answer.enabled;
        }

        let enabled = match flags.load(user).await {
            Ok(snapshot) => {
                let enabled = snapshot.is_enabled(self.flag);
                self.answers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .put(
                        user.clone(),
                        Answer {
                            enabled,
                            read_at: Instant::now(),
                        },
                    );
                span.record("cache", "miss");
                enabled
            }
            Err(error) => {
                // Keep the last answer for another interval, so an outage
                // doesn't add PostHog's timeout to every build and run.
                if let Some(answer) = known {
                    self.answers
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .put(
                            user.clone(),
                            Answer {
                                read_at: Instant::now(),
                                ..answer
                            },
                        );
                }
                let last = known.is_some_and(|answer| answer.enabled);
                tracing::warn!(
                    ?error,
                    reused = known.is_some(),
                    flag = self.flag,
                    "could not read a feature rollout"
                );
                span.record("cache", if known.is_some() { "stale" } else { "failed" });
                last
            }
        };
        span.record("allowed", enabled);
        enabled
    }
}

#[async_trait]
impl<F: FeatureFlags + 'static> DocumentProgramGate for FlaggedRollout<F> {
    async fn allows(&self, user: &MacroUserIdStr<'static>) -> bool {
        FlaggedRollout::allows(self, user).await
    }
}

/// The marketplace's listings are pages with programs, so it admits a user
/// only where both its own rollout and the document programs' do.
pub struct MarketplaceRollout<F> {
    marketplace: FlaggedRollout<F>,
    document_programs: Arc<FlaggedRollout<F>>,
}

impl<F> MarketplaceRollout<F> {
    /// Bind the marketplace's rollout and the document programs' shared one.
    pub fn new(marketplace: FlaggedRollout<F>, document_programs: Arc<FlaggedRollout<F>>) -> Self {
        Self {
            marketplace,
            document_programs,
        }
    }
}

impl<F: FeatureFlags + 'static> marketplace::domain::ports::Rollout for MarketplaceRollout<F> {
    async fn allows(&self, user: &MacroUserIdStr<'static>) -> bool {
        self.document_programs.allows(user).await && self.marketplace.allows(user).await
    }
}

#[cfg(test)]
mod test;
