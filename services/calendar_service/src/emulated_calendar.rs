//! One calendar connected to a provider emulator (`crates/gmail_emulator`,
//! `crates/outlook_emulator`) and kept in sync by the production sync code.
//!
//! The emulator plays the provider: its owner adds, moves and deletes events
//! there, and tests make it forget sync tokens the way the provider does.
//! The real Google backfill coordinator and Outlook calendar sync store those
//! events in Postgres, and the real mutation service writes Macro's own
//! events back. Specs then check what the owner would see in Macro's
//! calendar (the occurrences its read path lists) and in the provider's.
//!
//! Real: Postgres, Redis (token cache), the provider adapters, the sync and
//! mutation code, the authentication-service routes the emulators stand in
//! for. Faked: SQS (a sync runs the job the outbox would publish), Kafka and
//! the connection gateway (nobody listens).

mod google;
mod outlook;

use std::sync::Arc;

use calendar_events::domain::models::{CalendarEventDraft, EventTime, OccurrenceRange};
use calendar_events::domain::ports::CalendarRefreshNotifier;
use calendar_events::domain::service::CalendarService;
use calendar_events::outbound::pg::PgCalendarRepository;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::calendar_tokens::CalendarTokenProviderAdapter;

/// One occurrence as the owner sees it in Macro's calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub title: String,
    pub starts_at: DateTime<Utc>,
}

/// Every occurrence Macro shows `owner` from a month back to two months
/// ahead, in start order, through the calendar's own read path.
pub async fn shown(db: &PgPool, owner: &str) -> Vec<Shown> {
    let now = Utc::now();
    let starts_at = now - Duration::days(30);
    let ends_at = now + Duration::days(60);
    let range = OccurrenceRange {
        starts_at,
        ends_at,
        start_date: starts_at.date_naive(),
        end_date: ends_at.date_naive(),
    };
    CalendarService::new(PgCalendarRepository::new(db.clone()))
        .list_occurrences(owner, range, None, 2000)
        .await
        .expect("list the owner's occurrences")
        .into_iter()
        .map(|listing| listing.into_occurrence_event())
        .filter(|(_, occurrence)| !occurrence.is_cancelled)
        .map(|(event, occurrence)| Shown {
            title: event.title,
            starts_at: match occurrence.time {
                EventTime::Timed { starts_at, .. } => starts_at,
                EventTime::AllDay { start_date, .. } => {
                    start_date.and_hms_opt(0, 0, 0).unwrap().and_utc()
                }
            },
        })
        .collect()
}

/// The times of the occurrences titled `title` that Macro shows.
pub fn times_of(shown: &[Shown], title: &str) -> Vec<DateTime<Utc>> {
    shown
        .iter()
        .filter(|occurrence| occurrence.title == title)
        .map(|occurrence| occurrence.starts_at)
        .collect()
}

/// Ten in the morning UTC, `days` from today: inside every window the sync
/// and the read path keep.
pub fn day(days: i64) -> DateTime<Utc> {
    (Utc::now() + Duration::days(days))
        .date_naive()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_utc()
}

/// Production token acquisition against an emulator's stand-in for the
/// authentication service.
pub async fn tokens(authentication_service_url: String) -> CalendarTokenProviderAdapter {
    let redis_uri =
        std::env::var("REDIS_URI").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis = redis::Client::open(redis_uri)
        .expect("redis client")
        .get_multiplexed_async_connection()
        .await
        .expect("redis connection");
    CalendarTokenProviderAdapter::new(
        redis,
        Arc::new(authentication_service_client::AuthServiceClient::new(
            "secret".to_string(),
            authentication_service_url,
        )),
    )
}

/// Nobody has the calendar open.
#[derive(Clone, Copy)]
pub struct NoViewers;

impl CalendarRefreshNotifier for NoViewers {
    async fn calendar_changed(&self, _owner_id: &str, _email_link_id: Uuid, _team: bool) {}
    async fn join_request_answered(&self, _requester_id: &str, _event_id: Uuid) {}
}

/// A one-hour event the owner fills in on Macro's new-event form.
pub fn draft(title: &str, starts_at: DateTime<Utc>) -> CalendarEventDraft {
    CalendarEventDraft {
        idempotency_key: None,
        title: title.to_string(),
        description: None,
        location: None,
        time: EventTime::Timed {
            starts_at,
            ends_at: starts_at + Duration::hours(1),
            time_zone: Some("UTC".to_string()),
        },
        attendees: Vec::new(),
        recurrence_lines: Vec::new(),
        visibility: None,
        transparency: None,
        reminders: None,
        conference: None,
        out_of_office: None,
        guest_policy: None,
    }
}
