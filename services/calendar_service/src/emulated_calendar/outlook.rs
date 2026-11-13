//! An Outlook calendar over `outlook_emulator`, synced by the Outlook
//! calendar sync loop (`OutlookCalendarSync`) the service runs.

use std::sync::Mutex;

use calendar_events::domain::mutations::CalendarMutationServiceImpl;
use calendar_events::domain::outlook::OutlookCalendarSync;
use calendar_events::domain::ports::CalendarMutationService;
use calendar_events::domain::providers::CalendarProviders;
use calendar_events::outbound::google::{GoogleCalendarClient, UnmeteredGate};
use calendar_events::outbound::pg::PgCalendarRepository;
use chrono::{DateTime, Datelike, Duration, Utc};
use email_api_client::OutlookApiClientRepository;
use macro_event_broker::NoopMacroEventBroker;
use macro_service_urls::MicrosoftGraphUrl;
use outlook_emulator::{NewEvent, OutlookEmulator, Seed};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{NoViewers, Shown};
use crate::calendar_tokens::CalendarTokenProviderAdapter;

#[cfg(test)]
mod specs;

const OWNER: &str = "owner@example.com";
const DEFAULT: &str = "default";

/// More claims than any spec needs; a sync that doesn't settle by then is
/// looping.
const MAX_CLAIMS: usize = 500;

type Sync = OutlookCalendarSync<
    PgCalendarRepository,
    OutlookApiClientRepository,
    CalendarTokenProviderAdapter,
    NoViewers,
>;

type Mutations = CalendarMutationServiceImpl<
    PgCalendarRepository,
    CalendarProviders<GoogleCalendarClient<UnmeteredGate>, OutlookApiClientRepository>,
    CalendarTokenProviderAdapter,
    NoopMacroEventBroker,
    NoViewers,
>;

/// An event Outlook holds, by its id.
#[derive(Clone, Debug)]
pub struct Event {
    pub id: String,
}

/// A weekly series Outlook holds: its id and its first start.
#[derive(Clone, Debug)]
pub struct Series {
    pub id: String,
    pub first: DateTime<Utc>,
}

pub struct OutlookCalendar {
    pub db: PgPool,
    pub link_id: Uuid,
    owner_id: String,
    outlook: OutlookEmulator,
    sync: Sync,
    mutations: Mutations,
    errors: Mutex<Vec<String>>,
}

impl OutlookCalendar {
    /// An Outlook inbox whose owner granted calendar access when connecting.
    pub async fn connected(db: PgPool) -> OutlookCalendar {
        let outlook = OutlookEmulator::start(Seed::with_mailboxes([OWNER]))
            .await
            .expect("Outlook emulator");
        let link_id = Uuid::now_v7();
        outlook
            .link_mailbox(&link_id.to_string(), OWNER)
            .expect("link the mailbox");
        let owner_id = format!("macro|{OWNER}");
        sqlx::query(
            "INSERT INTO email_links (id, macro_id, fusionauth_user_id, email_address, provider, sync_generation, grant_generation, is_sync_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'OUTLOOK', 1, 1, true, NOW(), NOW())",
        )
        .bind(link_id)
        .bind(&owner_id)
        .bind(format!("fa-{link_id}"))
        .bind(OWNER)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO email_link_microsoft_scopes (link_id, grant_generation, granted_scopes)
             VALUES ($1, 1, ARRAY['Mail.ReadWrite', 'Mail.Send', 'Calendars.ReadWrite'])",
        )
        .bind(link_id)
        .execute(&db)
        .await
        .unwrap();

        let graph = || {
            OutlookApiClientRepository::for_graph_url(&MicrosoftGraphUrl::from(outlook.graph_url()))
                .expect("outlook client")
        };
        let tokens = super::tokens(outlook.authentication_service_url()).await;
        let repository = PgCalendarRepository::new(db.clone());
        let sync = OutlookCalendarSync::new(repository.clone(), graph(), tokens.clone(), NoViewers);
        let mutations = CalendarMutationServiceImpl::new(
            repository,
            CalendarProviders {
                google: GoogleCalendarClient::new_with_url(
                    reqwest::Client::new(),
                    "http://127.0.0.1:9/calendar/v3".to_string(),
                ),
                outlook: graph(),
            },
            tokens,
            NoopMacroEventBroker,
            NoViewers,
        )
        .with_outlook_writes_enabled(true);
        OutlookCalendar {
            db,
            link_id,
            owner_id,
            outlook,
            sync,
            mutations,
            errors: Mutex::new(Vec::new()),
        }
    }

    /// The owner puts a one-hour event on their calendar in Outlook.
    pub fn has_event(&self, title: &str, starts_at: DateTime<Utc>) -> Event {
        Event {
            id: self.add(json!({
                "subject": title,
                "start": at(starts_at),
                "end": at(starts_at + Duration::hours(1)),
            })),
        }
    }

    /// The owner puts a one-hour event repeating weekly, four times, on
    /// their calendar in Outlook.
    pub fn has_weekly_series(&self, title: &str, first: DateTime<Utc>) -> Series {
        let weekday = first.weekday().to_string().to_lowercase();
        let day = match weekday.as_str() {
            "mon" => "monday",
            "tue" => "tuesday",
            "wed" => "wednesday",
            "thu" => "thursday",
            "fri" => "friday",
            "sat" => "saturday",
            _ => "sunday",
        };
        let id = self.add(json!({
            "subject": title,
            "start": at(first),
            "end": at(first + Duration::hours(1)),
            "recurrence": {
                "pattern": {"type": "weekly", "interval": 1, "daysOfWeek": [day]},
                "range": {
                    "type": "numbered",
                    "startDate": first.date_naive().to_string(),
                    "numberOfOccurrences": 4,
                },
            },
        }));
        Series { id, first }
    }

    /// The owner renames an event and drags it to `starts_at` in Outlook.
    pub fn renames_and_moves(&self, event: &Event, title: &str, starts_at: DateTime<Utc>) {
        self.change(&event.id, title, starts_at);
    }

    /// The owner renames and moves one occurrence of a series ("just this
    /// one").
    pub fn changes_one_occurrence(
        &self,
        series: &Series,
        week: i64,
        title: &str,
        starts_at: DateTime<Utc>,
    ) {
        self.change(&self.occurrence(series, week), title, starts_at);
    }

    /// The owner deletes one occurrence of a series ("just this one").
    pub fn cancels_one_occurrence(&self, series: &Series, week: i64) {
        self.outlook
            .delete_event(OWNER, &self.occurrence(series, week))
            .expect("delete the occurrence");
    }

    /// The owner deletes an event in Outlook.
    pub fn deletes(&self, event: &Event) {
        self.outlook
            .delete_event(OWNER, &event.id)
            .expect("delete the event");
    }

    /// The owner deletes a whole series in Outlook.
    pub fn deletes_series(&self, series: &Series) {
        self.outlook
            .delete_event(OWNER, &series.id)
            .expect("delete the series");
    }

    /// The owner drags a whole series ("the entire series") later by `by`.
    pub fn moves_series(&self, series: &Series, by: Duration) {
        let first = series.first + by;
        self.outlook
            .update_event(
                OWNER,
                &series.id,
                &new_event(json!({
                    "start": at(first),
                    "end": at(first + Duration::hours(1)),
                })),
            )
            .expect("move the series");
    }

    /// The event titled `title` on the owner's calendar in Outlook.
    pub fn outlook_event(&self, title: &str) -> Event {
        let events = self
            .outlook
            .events(OWNER, DEFAULT)
            .expect("the calendar's events");
        let event = events
            .iter()
            .find(|event| event["subject"] == title)
            .unwrap_or_else(|| panic!("no {title} in {events:#?}"));
        Event {
            id: event["id"].as_str().expect("an event id").to_string(),
        }
    }

    /// Graph drops the mailbox's sync state: the next delta round answers
    /// 410 `SyncStateNotFound`.
    pub fn outlook_forgets_the_sync_state(&self) {
        self.outlook
            .expire_delta_tokens(OWNER)
            .expect("expire delta tokens");
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

    /// The titles of the events on the owner's calendar in Outlook.
    pub fn outlook_shows(&self) -> Vec<String> {
        self.outlook
            .events(OWNER, DEFAULT)
            .expect("the calendar's events")
            .into_iter()
            .filter(|event| event["isCancelled"] != true)
            .filter_map(|event| event["subject"].as_str().map(str::to_owned))
            .collect()
    }

    /// The two-minute poll comes round for every calendar of this inbox, and
    /// the sync loop claims work until none is left. A failed claim backs
    /// off, as the loop leaves it, and is reported by [`Self::sync_errors`].
    pub async fn syncs(&self) {
        sqlx::query(
            "UPDATE calendar_outlook_work SET next_run_at = now()
             WHERE account_id IN (SELECT id FROM calendar_accounts WHERE email_link_id = $1)",
        )
        .bind(self.link_id)
        .execute(&self.db)
        .await
        .unwrap();
        for _ in 0..MAX_CLAIMS {
            match self.sync.run_once().await {
                Ok(true) => {}
                Ok(false) => return,
                Err(error) => self.errors.lock().unwrap().push(format!("{error:?}")),
            }
        }
        panic!("Outlook calendar sync did not settle");
    }

    /// What the sync loop failed with so far.
    pub fn sync_errors(&self) -> Vec<String> {
        self.errors.lock().unwrap().clone()
    }

    /// Whether Graph answered a sync with 410, asking for a full one.
    pub fn outlook_asked_for_a_full_sync(&self) -> bool {
        self.outlook
            .requests()
            .iter()
            .any(|request| request.status == 410)
    }

    /// Every occurrence Macro shows the owner.
    pub async fn shows(&self) -> Vec<Shown> {
        super::shown(&self.db, &self.owner_id).await
    }

    fn occurrence(&self, series: &Series, week: i64) -> String {
        let date = (series.first + Duration::weeks(week)).date_naive();
        self.outlook.occurrence_id(&series.id, date)
    }

    fn add(&self, event: Value) -> String {
        self.outlook
            .add_event(OWNER, DEFAULT, &new_event(event))
            .expect("add the event")
    }

    fn change(&self, id: &str, title: &str, starts_at: DateTime<Utc>) {
        self.outlook
            .update_event(
                OWNER,
                id,
                &new_event(json!({
                    "subject": title,
                    "start": at(starts_at),
                    "end": at(starts_at + Duration::hours(1)),
                })),
            )
            .expect("change the event");
    }
}

fn at(instant: DateTime<Utc>) -> Value {
    json!({ "dateTime": instant.format("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": "UTC" })
}

fn new_event(event: Value) -> NewEvent {
    serde_json::from_value(event).expect("a Graph event")
}
