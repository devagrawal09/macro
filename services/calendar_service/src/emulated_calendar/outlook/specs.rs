//! How an Outlook calendar keeps up with Outlook.

use super::super::{day, times_of};
use super::*;
use macro_db_migrator::MACRO_DB_MIGRATIONS;

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_new_outlook_event_shows_up(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    calendar.syncs().await;
    calendar.has_event("Dentist", day(3));

    calendar.syncs().await;

    assert_eq!(
        times_of(&calendar.shows().await, "Dentist"),
        [day(3)],
        "{:?}",
        calendar.sync_errors()
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_event_renamed_and_moved_in_outlook_updates(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let review = calendar.has_event("Review", day(3));
    calendar.syncs().await;

    calendar.renames_and_moves(&review, "Design review", day(4));
    calendar.syncs().await;

    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert_eq!(
        times_of(&shown, "Design review"),
        [day(4)],
        "{shown:#?} {errors:?}"
    );
    assert!(times_of(&shown, "Review").is_empty(), "{shown:#?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn one_changed_occurrence_of_a_weekly_series_changes_alone(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let standup = calendar.has_weekly_series("Standup", day(1));
    calendar.syncs().await;

    let moved = day(9) + Duration::hours(2);
    calendar.changes_one_occurrence(&standup, 1, "Standup (moved)", moved);
    calendar.syncs().await;

    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert_eq!(
        times_of(&shown, "Standup"),
        [day(1), day(15), day(22)],
        "{shown:#?} {errors:?}"
    );
    assert_eq!(times_of(&shown, "Standup (moved)"), [moved], "{shown:#?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn one_cancelled_occurrence_of_a_weekly_series_disappears_alone(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let standup = calendar.has_weekly_series("Standup", day(1));
    calendar.syncs().await;

    calendar.cancels_one_occurrence(&standup, 2);
    calendar.syncs().await;

    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert_eq!(
        times_of(&shown, "Standup"),
        [day(1), day(8), day(22)],
        "{shown:#?} {errors:?}"
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_event_deleted_in_outlook_disappears(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let dentist = calendar.has_event("Dentist", day(3));
    calendar.has_event("Lunch", day(3) + Duration::hours(2));
    calendar.syncs().await;

    calendar.deletes(&dentist);
    calendar.syncs().await;

    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert!(
        times_of(&shown, "Dentist").is_empty(),
        "{shown:#?} {errors:?}"
    );
    assert_eq!(times_of(&shown, "Lunch").len(), 1, "{shown:#?}");
}

/// Graph answers 410 for the old delta link; the sync reads everything
/// again. What changed meanwhile lands, and nothing shows twice.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn the_calendar_stays_whole_after_outlook_forgets_the_sync_state(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    calendar.has_event("Lunch", day(2));
    let dentist = calendar.has_event("Dentist", day(3));
    calendar.has_weekly_series("Standup", day(1));
    calendar.syncs().await;

    calendar.outlook_forgets_the_sync_state();
    calendar.deletes(&dentist);
    calendar.has_event("Offsite", day(5));
    calendar.syncs().await;

    assert!(calendar.outlook_asked_for_a_full_sync());
    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert_eq!(times_of(&shown, "Lunch"), [day(2)], "{shown:#?} {errors:?}");
    assert!(times_of(&shown, "Dentist").is_empty(), "{shown:#?}");
    assert_eq!(times_of(&shown, "Offsite"), [day(5)], "{shown:#?}");
    assert_eq!(
        times_of(&shown, "Standup"),
        [day(1), day(8), day(15), day(22)],
        "{shown:#?}"
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_event_created_in_macro_reaches_outlook(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    calendar.syncs().await;

    calendar.creates_in_macro("Planning", day(6)).await;
    calendar.syncs().await;

    assert_eq!(calendar.outlook_shows(), ["Planning"]);
    let shown = calendar.shows().await;
    let errors = calendar.sync_errors();
    assert_eq!(
        times_of(&shown, "Planning"),
        [day(6)],
        "{shown:#?} {errors:?}"
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_series_deleted_in_outlook_disappears_entirely(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let standup = calendar.has_weekly_series("Standup", day(1));
    calendar.changes_one_occurrence(&standup, 1, "Standup (moved)", day(9));
    calendar.syncs().await;

    calendar.deletes_series(&standup);
    calendar.syncs().await;

    let shown = calendar.shows().await;
    assert!(shown.is_empty(), "{shown:#?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_series_moved_in_outlook_moves_every_occurrence(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let standup = calendar.has_weekly_series("Standup", day(1));
    calendar.syncs().await;

    calendar.moves_series(&standup, Duration::hours(3));
    calendar.syncs().await;

    let later = |week: i64| day(1 + 7 * week) + Duration::hours(3);
    let shown = calendar.shows().await;
    assert_eq!(
        times_of(&shown, "Standup"),
        [later(0), later(1), later(2), later(3)],
        "{shown:#?}"
    );
}

/// The full listing after a 410 must keep what the owner did to single
/// occurrences before it.
#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn changed_and_cancelled_occurrences_survive_a_full_resync(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    let standup = calendar.has_weekly_series("Standup", day(1));
    calendar.syncs().await;
    let moved = day(9) + Duration::hours(2);
    calendar.changes_one_occurrence(&standup, 1, "Standup (moved)", moved);
    calendar.cancels_one_occurrence(&standup, 2);
    calendar.syncs().await;

    calendar.outlook_forgets_the_sync_state();
    calendar.syncs().await;

    assert!(calendar.outlook_asked_for_a_full_sync());
    let shown = calendar.shows().await;
    assert_eq!(times_of(&shown, "Standup"), [day(1), day(22)], "{shown:#?}");
    assert_eq!(times_of(&shown, "Standup (moved)"), [moved], "{shown:#?}");
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_event_created_in_macro_then_renamed_in_outlook_shows_once_renamed(pool: PgPool) {
    let calendar = OutlookCalendar::connected(pool).await;
    calendar.syncs().await;
    calendar.creates_in_macro("Planning", day(6)).await;
    calendar.syncs().await;

    let planning = calendar.outlook_event("Planning");
    calendar.renames_and_moves(&planning, "Planning (final)", day(7));
    calendar.syncs().await;

    let shown = calendar.shows().await;
    assert_eq!(shown.len(), 1, "{shown:#?}");
    assert_eq!(times_of(&shown, "Planning (final)"), [day(7)], "{shown:#?}");
}
