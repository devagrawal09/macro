//! Routine listings: publishing, the forced major, installing with fills and
//! keeping installs up to date.

use std::collections::BTreeMap;

use chrono_tz::Tz;
use scheduled_action::domain::templates::VariableFill;
use serde_json::{Value, json};

use super::*;
use crate::domain::service::routine_bump;

const DIGEST: Uuid = Uuid::from_u128(20);

const BOARD_MENTION: &str = r#"<m-document-mention>{"documentId":"board","documentName":"Board","blockName":"md"}</m-document-mention>"#;

fn routine_receipt(id: &'static str, routine: Uuid) -> EntityAccessReceipt<OwnerAccessLevel> {
    EntityAccessReceipt::dangerously_assert_authenticated_user(
        user(id),
        &routine.to_string(),
        EntityType::ScheduledAction,
    )
}

fn routine_source(id: &'static str, variables: Vec<TemplateVariable>) -> PublishSource {
    PublishSource::Routine {
        routine: routine_receipt(id, DIGEST),
        variables,
    }
}

fn board(fill: VariableFill) -> TemplateVariable {
    TemplateVariable::Item {
        id: "board".to_owned(),
        block_name: "md".to_owned(),
        label: "Board".to_owned(),
        fill,
    }
}

fn zone() -> TemplateVariable {
    TemplateVariable::TimeZone {
        timezone: Tz::Europe__London,
        fill: VariableFill::Activator,
    }
}

fn digest(instructions: &str) -> RoutineSnapshot {
    RoutineSnapshot {
        name: "Weekly digest".to_owned(),
        description: String::new(),
        trigger: serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "Europe/London",
        }))
        .unwrap(),
        task: json!({ "steps": [{ "type": "agent", "instructions": instructions }] }),
        variables: Vec::new(),
    }
}

fn instructions(routine: &SavedRoutine) -> Value {
    routine.routine.task["steps"][0]["instructions"].clone()
}

fn in_new_york() -> InstallRequest {
    InstallRequest {
        timezone: Some(Tz::America__New_York),
        ..InstallRequest::default()
    }
}

/// Ada, a member of Acme, publishes her weekly digest; its time zone fills
/// in as whoever installs it.
async fn published_digest(service: &Service, routines: &FakeRoutines) -> Listing {
    routines.save_source(ADA, DIGEST, digest("Sum up the week"));
    service
        .publish(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            routine_source(ADA, vec![zone()]),
            ListingText {
                title: "Weekly digest".to_owned(),
                description: "Sums up the week".to_owned(),
            },
        )
        .await
        .expect("the digest publishes")
}

/// Ada changes her digest's instructions and publishes them as a version.
async fn publish_instructions(
    service: &Service,
    routines: &FakeRoutines,
    listing: ListingId,
    text: &str,
    variables: Vec<TemplateVariable>,
    bump: VersionBump,
) -> PublishedVersion {
    routines.save_source(ADA, DIGEST, digest(text));
    service
        .publish_version(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            listing,
            routine_source(ADA, variables),
            bump,
        )
        .await
        .expect("the version publishes")
}

async fn install(service: &Service, id: &'static str, listing: ListingId) -> Listing {
    service
        .install(
            &user(id),
            Some(&member(id, ACME, TeamRole::Member)),
            listing,
            in_new_york(),
        )
        .await
        .expect("the routine installs")
}

fn installed_id(listing: &Listing) -> Uuid {
    listing
        .installed_routine
        .as_ref()
        .expect("an installed routine")
        .routine_id
}

#[tokio::test]
async fn a_routine_publishes_as_one_point_zero_with_its_snapshot() {
    let (service, _, routines) = marketplace_with_routines();

    let listing = published_digest(&service, &routines).await;
    let newest = service
        .routine_version(
            &user(BEN),
            Some(&member(BEN, ACME, TeamRole::Member)),
            listing.id,
        )
        .await
        .expect("the teammate reads it");
    let not_hers = service
        .publish(
            &user(BEN),
            Some(&member(BEN, ACME, TeamRole::Member)),
            routine_source(BEN, Vec::new()),
            text("Someone else's digest"),
        )
        .await;

    assert_eq!(listing.kind, ListingKind::Routine);
    assert_eq!(listing.source_id, DIGEST);
    assert_eq!(listing.latest_version, Version { major: 1, minor: 0 });
    assert_eq!(newest.version, Version { major: 1, minor: 0 });
    assert_eq!(newest.snapshot.description, "Sums up the week");
    assert_eq!(newest.snapshot.variables, vec![zone()]);
    assert!(matches!(not_hers, Err(MarketplaceError::NotFound)));
}

#[test]
fn a_changed_interface_forces_a_major_version() {
    let before = digest("Sum up the week");
    let reworded = digest("Sum up the week in three lines");
    let with_variable = RoutineSnapshot {
        variables: vec![board(VariableFill::Choice)],
        ..before.clone()
    };

    assert_eq!(
        routine_bump(VersionBump::Minor, &before, &reworded),
        VersionBump::Minor
    );
    assert_eq!(
        routine_bump(VersionBump::Major, &before, &reworded),
        VersionBump::Major
    );
    assert_eq!(
        routine_bump(VersionBump::Minor, &before, &with_variable),
        VersionBump::Major
    );
}

#[tokio::test]
async fn a_minor_version_with_a_new_variable_is_published_as_a_major() {
    let (service, _, routines) = marketplace_with_routines();
    let listing = published_digest(&service, &routines).await;

    let published = publish_instructions(
        &service,
        &routines,
        listing.id,
        &format!("Sum up {BOARD_MENTION}"),
        vec![zone(), board(VariableFill::Shared)],
        VersionBump::Minor,
    )
    .await;

    assert_eq!(published.bump, VersionBump::Major);
    assert_eq!(
        published.listing.latest_version,
        Version { major: 2, minor: 0 }
    );
}

#[tokio::test]
async fn installing_fills_in_the_routine_as_the_persons_own() {
    let (service, _, routines) = marketplace_with_routines();
    routines.save_source(ADA, DIGEST, digest(&format!("Sum up {BOARD_MENTION}")));
    let listing = service
        .publish(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            routine_source(ADA, vec![zone(), board(VariableFill::Choice)]),
            text("Weekly digest"),
        )
        .await
        .expect("the digest publishes");
    let ben = member(BEN, ACME, TeamRole::Member);

    let without_fills = service
        .install(&user(BEN), Some(&ben), listing.id, in_new_york())
        .await;
    let without_zone = service
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest {
                variables: BTreeMap::from([("board".to_owned(), VariableFill::Choice)]),
                ..InstallRequest::default()
            },
        )
        .await;
    let installed = service
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest {
                variables: BTreeMap::from([("board".to_owned(), VariableFill::Choice)]),
                ..in_new_york()
            },
        )
        .await
        .expect("the routine installs");
    let saved = routines.saved(installed_id(&installed));

    match without_fills {
        Err(MarketplaceError::MissingFills(missing)) => {
            assert_eq!(missing, vec![board(VariableFill::Choice)]);
        }
        other => panic!("expected missing fills, got {other:?}"),
    }
    assert!(matches!(without_zone, Err(MarketplaceError::Invalid(_))));
    assert_eq!(installed.installed_major, Some(1));
    assert_eq!(saved.owner, BEN);
    assert!(!saved.enabled);
    // Left out, the reference reads as its name.
    assert_eq!(instructions(&saved), json!("Sum up Board"));
    assert_eq!(
        saved.routine.trigger,
        serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "America/New_York",
        }))
        .unwrap()
    );
}

#[tokio::test]
async fn a_minor_version_updates_installed_routines() {
    let (service, _, routines) = marketplace_with_routines();
    let listing = published_digest(&service, &routines).await;
    let installed = install(&service, BEN, listing.id).await;
    let routine_id = installed_id(&installed);

    let published = publish_instructions(
        &service,
        &routines,
        listing.id,
        "Sum up the week in three lines",
        vec![zone()],
        VersionBump::Minor,
    )
    .await;
    let saved = routines.saved(routine_id);

    assert_eq!(published.bump, VersionBump::Minor);
    assert_eq!(published.updates.updated, 1);
    assert_eq!(
        instructions(&saved),
        json!("Sum up the week in three lines")
    );
    assert_eq!(saved.revision, 2);
    // Filled in again from the fills stored at install.
    assert_eq!(
        saved.routine.trigger,
        serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "America/New_York",
        }))
        .unwrap()
    );
}

#[tokio::test]
async fn an_edited_routine_is_detached_and_a_failure_blocks_no_one_else() {
    let (service, _, routines) = marketplace_with_routines();
    let listing = published_digest(&service, &routines).await;
    let bens = installed_id(&install(&service, BEN, listing.id).await);
    let cams = installed_id(&install(&service, CAM, listing.id).await);
    let adas = installed_id(&install(&service, ADA, listing.id).await);
    routines.edit(
        bens,
        json!({ "steps": [{ "type": "agent", "instructions": "My own words" }] }),
    );
    routines.fail(cams);

    let published = publish_instructions(
        &service,
        &routines,
        listing.id,
        "Sum up the week in three lines",
        vec![zone()],
        VersionBump::Minor,
    )
    .await;
    let for_ben = service
        .listings(&user(BEN), Some(&member(BEN, ACME, TeamRole::Member)))
        .await
        .expect("listings load");
    let for_cam = service
        .listings(&user(CAM), Some(&member(CAM, ACME, TeamRole::Member)))
        .await
        .expect("listings load");

    assert_eq!(published.updates.updated, 1);
    assert_eq!(published.updates.detached, 1);
    assert_eq!(published.updates.failed, 1);
    assert_eq!(instructions(&routines.saved(bens)), json!("My own words"));
    assert_eq!(routines.saved(bens).revision, 1);
    assert_eq!(
        instructions(&routines.saved(adas)),
        json!("Sum up the week in three lines")
    );
    assert!(
        for_ben[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| routine.detached)
    );
    assert!(
        for_cam[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| { routine.update_failed && !routine.detached })
    );
}

#[tokio::test]
async fn a_deleted_routine_records_a_failure_and_stays_attached() {
    let (service, _, routines) = marketplace_with_routines();
    let listing = published_digest(&service, &routines).await;
    let bens = installed_id(&install(&service, BEN, listing.id).await);
    routines.delete(bens);

    let published = publish_instructions(
        &service,
        &routines,
        listing.id,
        "Sum up the week in three lines",
        vec![zone()],
        VersionBump::Minor,
    )
    .await;
    let for_ben = service
        .listings(&user(BEN), Some(&member(BEN, ACME, TeamRole::Member)))
        .await
        .expect("listings load");

    assert_eq!(published.updates.failed, 1);
    assert_eq!(published.updates.detached, 0);
    assert!(
        for_ben[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| { routine.update_failed && !routine.detached })
    );
}

#[tokio::test]
async fn a_major_version_asks_for_new_fills_before_it_updates() {
    let (service, _, routines) = marketplace_with_routines();
    let listing = published_digest(&service, &routines).await;
    let routine_id = installed_id(&install(&service, BEN, listing.id).await);
    let ben = member(BEN, ACME, TeamRole::Member);
    publish_instructions(
        &service,
        &routines,
        listing.id,
        &format!("Sum up {BOARD_MENTION}"),
        vec![zone(), board(VariableFill::Choice)],
        VersionBump::Major,
    )
    .await;

    let waiting = routines.saved(routine_id);
    let without_fill = service
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
        .await;
    let updated = service
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest {
                variables: BTreeMap::from([("board".to_owned(), VariableFill::Shared)]),
                ..InstallRequest::default()
            },
        )
        .await
        .expect("the major version is taken");
    let saved = routines.saved(routine_id);

    assert_eq!(instructions(&waiting), json!("Sum up the week"));
    match without_fill {
        Err(MarketplaceError::MissingFills(missing)) => {
            assert_eq!(missing, vec![board(VariableFill::Choice)]);
        }
        other => panic!("expected missing fills, got {other:?}"),
    }
    assert_eq!(updated.installed_major, Some(2));
    assert_eq!(installed_id(&updated), routine_id);
    assert_eq!(
        instructions(&saved),
        json!(format!("Sum up {BOARD_MENTION}"))
    );
    // The time zone stored at install still applies.
    assert_eq!(
        saved.routine.trigger,
        serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "America/New_York",
        }))
        .unwrap()
    );
}
