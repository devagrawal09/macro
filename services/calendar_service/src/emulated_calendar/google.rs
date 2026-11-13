//! A Google calendar over `gmail_emulator`, synced by the backfill
//! coordinator the calendar backfill worker runs (`calendar_backfill`).

use calendar_events::domain::models::{
    CalendarBackfillJobKey, CalendarBackfillKind, CalendarGrantIntent, GOOGLE_CALENDAR_SCOPES,
    GoogleBackfillRunReport, GoogleScopeSet, OccurrenceRange,
};
use calendar_events::domain::mutations::CalendarMutationServiceImpl;
use calendar_events::domain::ports::CalendarMutationService;
use calendar_events::domain::providers::CalendarProviders;
use calendar_events::domain::service::{
    CalendarService, GoogleCalendarBackfillCoordinator, GoogleCalendarSyncScheduler,
};
use calendar_events::outbound::google::{GoogleCalendarClient, UnmeteredGate};
use calendar_events::outbound::pg::PgCalendarRepository;
use chrono::{DateTime, Duration, Utc};
use email_api_client::OutlookApiClientRepository;
use gmail_emulator::{GmailEmulator, NewEvent, Seed};
use macro_event_broker::NoopMacroEventBroker;
use macro_service_urls::MicrosoftGraphUrl;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{NoViewers, Shown};
use crate::calendar_tokens::CalendarTokenProviderAdapter;

#[cfg(test)]
mod specs;

const OWNER: &str = "owner@example.com";
const PRIMARY: &str = "primary";

type Google = GoogleCalendarClient<UnmeteredGate>;

type Coordinator = GoogleCalendarBackfillCoordinator<
    PgCalendarRepository,
    Google,
    PgCalendarRepository,
    NoopMacroEventBroker,
>;

type Mutations = CalendarMutationServiceImpl<
    PgCalendarRepository,
    CalendarProviders<Google, OutlookApiClientRepository>,
    CalendarTokenProviderAdapter,
    NoopMacroEventBroker,
    NoViewers,
>;

/// An event Google holds, by its id.
#[derive(Clone, Debug)]
pub struct Event {
    pub id: String,
}

/// A weekly series Google holds: its id and its first start.
#[derive(Clone, Debug)]
pub struct Series {
    pub id: String,
    pub first: DateTime<Utc>,
}

impl Series {
    /// Google's id for the occurrence `week` weeks after the first.
    fn occurrence(&self, week: i64) -> String {
        let start = self.first + Duration::weeks(week);
        format!("{}_{}", self.id, start.format("%Y%m%dT%H%M%SZ"))
    }
}

pub struct GoogleCalendar {
    pub db: PgPool,
    pub link_id: Uuid,
    owner_id: String,
    google: GmailEmulator,
    coordinator: Coordinator,
    tokens: CalendarTokenProviderAdapter,
    mutations: Mutations,
}

impl GoogleCalendar {
    /// An inbox whose owner just granted calendar access: its grant is
    /// recorded, which queues the first calendar sync.
    pub async fn connected(db: PgPool) -> GoogleCalendar {
        let google = GmailEmulator::start(Seed::with_mailboxes([OWNER]))
            .await
            .expect("Gmail emulator");
        let link_id = Uuid::now_v7();
        let owner_id = format!("macro|{OWNER}");
        sqlx::query(
            "INSERT INTO email_links (id, macro_id, fusionauth_user_id, email_address, provider, is_sync_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'GMAIL', true, NOW(), NOW())",
        )
        .bind(link_id)
        .bind(&owner_id)
        .bind(format!("fa-{link_id}"))
        .bind(OWNER)
        .execute(&db)
        .await
        .unwrap();
        let grant = CalendarService::new(PgCalendarRepository::new(db.clone()))
            .apply_google_grant(
                link_id,
                GoogleScopeSet::from_scopes(GOOGLE_CALENDAR_SCOPES.map(str::to_owned)),
                CalendarGrantIntent::CalendarRequested,
            )
            .await
            .expect("record the grant");
        assert!(
            grant
                .jobs
                .iter()
                .any(|job| job.kind == CalendarBackfillKind::GoogleCalendar),
            "the grant queues a calendar sync"
        );

        let client = || Google::new_with_url(reqwest::Client::new(), google.calendar_url());
        let repository = PgCalendarRepository::new(db.clone());
        let coordinator = GoogleCalendarBackfillCoordinator::new(
            repository.clone(),
            client(),
            repository.clone(),
            NoopMacroEventBroker,
            None,
        );
        let tokens = super::tokens(google.authentication_service_url()).await;
        let mutations = CalendarMutationServiceImpl::new(
            repository,
            CalendarProviders {
                google: client(),
                outlook: OutlookApiClientRepository::for_graph_url(&MicrosoftGraphUrl::from(
                    "http://127.0.0.1:9/v1.0/".to_string(),
                ))
                .expect("outlook client"),
            },
            tokens.clone(),
            NoopMacroEventBroker,
            NoViewers,
        );
        GoogleCalendar {
            db,
            link_id,
            owner_id,
            google,
            coordinator,
            tokens,
            mutations,
        }
    }

    /// The owner puts a one-hour event on their calendar in Google Calendar.
    pub fn has_event(&self, title: &str, starts_at: DateTime<Utc>) -> Event {
        let event = self.add(json!({
            "summary": title,
            "start": at(starts_at),
            "end": at(starts_at + Duration::hours(1)),
        }));
        Event { id: id_of(&event) }
    }

    /// The owner puts a one-hour event repeating weekly, four times, on
    /// their calendar in Google Calendar.
    pub fn has_weekly_series(&self, title: &str, first: DateTime<Utc>) -> Series {
        let event = self.add(json!({
            "summary": title,
            "start": at(first),
            "end": at(first + Duration::hours(1)),
            "recurrence": ["RRULE:FREQ=WEEKLY;COUNT=4"],
        }));
        Series {
            id: id_of(&event),
            first,
        }
    }

    /// The owner renames an event and drags it to `starts_at` in Google
    /// Calendar.
    pub fn renames_and_moves(&self, event: &Event, title: &str, starts_at: DateTime<Utc>) {
        self.change(&event.id, title, starts_at);
    }

    /// The owner renames and moves one occurrence of a series ("this event
    /// only").
    pub fn changes_one_occurrence(
        &self,
        series: &Series,
        week: i64,
        title: &str,
        starts_at: DateTime<Utc>,
    ) {
        self.change(&series.occurrence(week), title, starts_at);
    }

    /// The owner deletes one occurrence of a series ("this event only").
    pub fn cancels_one_occurrence(&self, series: &Series, week: i64) {
        self.google
            .delete_event(OWNER, PRIMARY, &series.occurrence(week))
            .expect("delete the occurrence");
    }

    /// The owner deletes an event in Google Calendar.
    pub fn deletes(&self, event: &Event) {
        self.google
            .delete_event(OWNER, PRIMARY, &event.id)
            .expect("delete the event");
    }

    /// The owner deletes a whole series in Google Calendar.
    pub fn deletes_series(&self, series: &Series) {
        self.google
            .delete_event(OWNER, PRIMARY, &series.id)
            .expect("delete the series");
    }

    /// The owner drags a whole series ("all events") later by `by`.
    pub fn moves_series(&self, series: &Series, by: Duration) {
        let first = series.first + by;
        self.google
            .change_event(
                OWNER,
                PRIMARY,
                &series.id,
                NewEvent::from_json(json!({
                    "start": at(first),
                    "end": at(first + Duration::hours(1)),
                })),
            )
            .expect("move the series");
    }

    /// The event titled `title` on the owner's calendar in Google.
    pub fn google_event(&self, title: &str) -> Event {
        let events = self
            .google
            .events(OWNER, PRIMARY)
            .expect("the calendar's events");
        let event = events
            .iter()
            .find(|event| event["summary"] == title)
            .unwrap_or_else(|| panic!("no {title} in {events:#?}"));
        Event { id: id_of(event) }
    }

    /// Google drops the calendar's sync tokens: the next incremental listing
    /// answers 410 `fullSyncRequired`.
    pub fn google_forgets_the_sync_token(&self) {
        self.google
            .expire_sync_tokens(OWNER, PRIMARY)
            .expect("expire sync tokens");
    }

    /// The owner creates a one-hour event in Macro's calendar.
    pub async fn creates_in_macro(&self, title: &str, starts_at: DateTime<Utc>) {
        self.mutations
            .create_event(
                &self.owner_id,
                Some(self.link_id),
                None,
                super::draft(title, starts_at),
            )
            .await
            .expect("create the event");
    }

    /// The titles of the events on the owner's calendar in Google.
    pub fn google_shows(&self) -> Vec<String> {
        self.google
            .events(OWNER, PRIMARY)
            .expect("the calendar's events")
            .into_iter()
            .filter(|event| event["status"] != "cancelled")
            .filter_map(|event| event["summary"].as_str().map(str::to_owned))
            .collect()
    }

    /// The scheduled poll comes round: every finished sync of this inbox is
    /// due again, and the worker runs each job the outbox publishes, as
    /// often as the queue would redeliver a retryable failure.
    pub async fn syncs(&self) {
        // The scheduler re-arms accounts polled more than five minutes ago.
        GoogleCalendarSyncScheduler::new(PgCalendarRepository::new(self.db.clone()))
            .run_once(Utc::now() + Duration::minutes(10))
            .await
            .expect("schedule due syncs");
        let jobs: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE calendar_sync_outbox outbox SET published_at = now()
             FROM calendar_backfill_jobs job
             WHERE job.id = outbox.backfill_job_id AND job.email_link_id = $1
               AND job.kind = 'google_calendar' AND outbox.published_at IS NULL
             RETURNING outbox.backfill_job_id",
        )
        .bind(self.link_id)
        .fetch_all(&self.db)
        .await
        .unwrap();
        assert!(!jobs.is_empty(), "the poll queues a sync");
        for job_id in jobs {
            self.runs(job_id).await;
        }
    }

    async fn runs(&self, job_id: Uuid) {
        let key = CalendarBackfillJobKey {
            job_id,
            email_link_id: self.link_id,
        };
        let mut errors = Vec::new();
        for _ in 0..5 {
            let token =
                calendar_events::domain::ports::CalendarAccessTokenProvider::fetch_access_token(
                    &self.tokens,
                    &self.token_identity(),
                )
                .await
                .expect("access token");
            let mut report = GoogleBackfillRunReport::default();
            match self
                .coordinator
                .run(
                    key,
                    &self.owner_id,
                    &token,
                    OccurrenceRange::maintenance_horizon(Utc::now()),
                    &mut report,
                )
                .await
            {
                Ok(()) => return,
                Err(error) => errors.push(format!("{error:?}")),
            }
        }
        panic!("the calendar sync kept failing: {errors:#?}");
    }

    fn token_identity(&self) -> calendar_events::domain::models::CalendarLinkTokenIdentity {
        calendar_events::domain::models::CalendarLinkTokenIdentity {
            binding: None,
            fusionauth_user_id: format!("fa-{}", self.link_id),
            email_address: OWNER.to_string(),
            provider: calendar_events::domain::models::CalendarProvider::Google,
        }
    }

    /// Whether Google answered a sync with 410, asking for a full one.
    pub fn google_asked_for_a_full_sync(&self) -> bool {
        self.google
            .requests()
            .iter()
            .any(|request| request.status == 410)
    }

    /// Every occurrence Macro shows the owner.
    pub async fn shows(&self) -> Vec<Shown> {
        super::shown(&self.db, &self.owner_id).await
    }

    fn add(&self, event: Value) -> Value {
        self.google
            .add_event(OWNER, PRIMARY, NewEvent::from_json(event))
            .expect("add the event")
    }

    fn change(&self, id: &str, title: &str, starts_at: DateTime<Utc>) {
        self.google
            .change_event(
                OWNER,
                PRIMARY,
                id,
                NewEvent::from_json(json!({
                    "summary": title,
                    "start": at(starts_at),
                    "end": at(starts_at + Duration::hours(1)),
                })),
            )
            .expect("change the event");
    }
}

fn at(instant: DateTime<Utc>) -> Value {
    json!({ "dateTime": instant.to_rfc3339(), "timeZone": "UTC" })
}

fn id_of(event: &Value) -> String {
    event["id"].as_str().expect("an event id").to_string()
}
