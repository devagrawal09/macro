//! Global listings: who publishes them, the second publisher's review, and
//! how nothing reaches anyone else until a version is approved.

use chrono_tz::Tz;
use scheduled_action::domain::templates::VariableFill;
use serde_json::json;

use super::*;
use crate::domain::models::{
    InterfaceChange, InterfaceChangeKind, InterfacePart, ReviewOutcome, ReviewPreview,
};

/// Macro's publisher team: Ada and Ben.
const MACRO: Uuid = Uuid::from_u128(3);
const DIGEST: Uuid = Uuid::from_u128(30);

fn publisher(id: &'static str) -> EntityAccessReceipt<MemberTeamRole> {
    member(id, MACRO, TeamRole::Member)
}

/// Cam, a member of another team.
fn outsider() -> EntityAccessReceipt<MemberTeamRole> {
    member(CAM, ACME, TeamRole::Member)
}

fn global_marketplace() -> (
    Service,
    Arc<FakePages>,
    Arc<FakeRoutines>,
    Arc<FakeNotifier>,
) {
    let (service, pages, routines, notifier) = marketplace_with(FakeNotifier::default());
    (
        service.with_publisher_team(Some(MACRO)),
        pages,
        routines,
        notifier,
    )
}

/// Ada publishes her board to everyone; it waits for review.
async fn global_board(service: &Service, pages: &FakePages) -> Listing {
    pages.save(BOARD, "<h1>Board</h1>");
    service
        .publish(
            &user(ADA),
            Some(&publisher(ADA)),
            owned(ADA, BOARD),
            text("Board"),
            PublishScope::Global,
        )
        .await
        .expect("a publisher publishes to everyone")
}

async fn decide(
    service: &Service,
    reviewer: &'static str,
    listing: ListingId,
    version: Version,
    decision: ReviewDecision,
) -> Result<ReviewOutcome, MarketplaceError> {
    service
        .review(
            &user(reviewer),
            Some(&publisher(reviewer)),
            listing,
            version,
            decision,
        )
        .await
}

/// Ben approves the listing's pending `version`.
async fn approve(service: &Service, listing: ListingId, version: Version) -> ReviewOutcome {
    decide(service, BEN, listing, version, ReviewDecision::Approve)
        .await
        .expect("another publisher approves it")
}

async fn listed_for_cam(service: &Service) -> Vec<Listing> {
    service
        .listings(
            &user(CAM),
            Some(&outsider()),
            first_page(ListingsShown::Listed),
        )
        .await
        .expect("listings load")
        .page
        .listings
}

async fn publish_board_version(
    service: &Service,
    pages: &FakePages,
    listing: ListingId,
    html: &str,
    bump: VersionBump,
) -> PublishedVersion {
    pages.save(BOARD, html);
    service
        .publish_version(
            &user(ADA),
            Some(&publisher(ADA)),
            listing,
            owned(ADA, BOARD),
            bump,
        )
        .await
        .expect("the version publishes")
}

const V1_0: Version = Version { major: 1, minor: 0 };
const V1_1: Version = Version { major: 1, minor: 1 };

#[tokio::test]
async fn without_a_publisher_team_nobody_publishes_to_everyone() {
    let (service, pages) = marketplace();
    pages.save(BOARD, "<h1>Board</h1>");

    let refused = service
        .publish(
            &user(ADA),
            Some(&publisher(ADA)),
            owned(ADA, BOARD),
            text("Board"),
            PublishScope::Global,
        )
        .await;
    let flag = service
        .listings(
            &user(ADA),
            Some(&publisher(ADA)),
            first_page(ListingsShown::Listed),
        )
        .await
        .expect("listings load")
        .global_publisher;

    assert!(matches!(refused, Err(MarketplaceError::NotGlobalPublisher)));
    assert!(!flag);
}

#[tokio::test]
async fn only_the_publisher_team_publishes_to_everyone_and_reviews() {
    let (service, pages, _, _) = global_marketplace();
    pages.save(BOARD, "<h1>Board</h1>");

    let by_outsider = service
        .publish(
            &user(CAM),
            Some(&outsider()),
            owned(CAM, BOARD),
            text("Board"),
            PublishScope::Global,
        )
        .await;
    let without_team = service
        .publish(
            &user(CAM),
            None,
            owned(CAM, BOARD),
            text("Board"),
            PublishScope::Global,
        )
        .await;
    let listing = global_board(&service, &pages).await;
    let queue_for_outsider = service
        .pending_versions(&user(CAM), Some(&outsider()))
        .await;
    let preview_for_outsider = service
        .review_preview(&user(CAM), Some(&outsider()), listing.id)
        .await;
    let approved_by_outsider = service
        .review(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            V1_0,
            ReviewDecision::Approve,
        )
        .await;
    let flags = (
        service
            .listings(
                &user(BEN),
                Some(&publisher(BEN)),
                first_page(ListingsShown::Listed),
            )
            .await
            .expect("listings load")
            .global_publisher,
        service
            .listings(
                &user(CAM),
                Some(&outsider()),
                first_page(ListingsShown::Listed),
            )
            .await
            .expect("listings load")
            .global_publisher,
    );

    assert!(matches!(
        by_outsider,
        Err(MarketplaceError::NotGlobalPublisher)
    ));
    assert!(matches!(without_team, Err(MarketplaceError::NoTeam)));
    assert_eq!(listing.scope, ListingScope::Global);
    assert!(matches!(
        queue_for_outsider,
        Err(MarketplaceError::NotGlobalPublisher)
    ));
    assert!(matches!(
        preview_for_outsider,
        Err(MarketplaceError::NotGlobalPublisher)
    ));
    assert!(matches!(
        approved_by_outsider,
        Err(MarketplaceError::NotGlobalPublisher)
    ));
    assert_eq!(flags, (true, false));
}

#[tokio::test]
async fn a_new_global_listing_is_invisible_outside_the_publisher_team_until_approved() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;

    let before = listed_for_cam(&service).await;
    let install_before = service
        .install(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            InstallRequest::default(),
        )
        .await;
    let for_ben = service
        .listings(
            &user(BEN),
            Some(&publisher(BEN)),
            first_page(ListingsShown::Listed),
        )
        .await
        .expect("listings load")
        .page
        .listings;
    let install_by_publisher = service
        .install(
            &user(BEN),
            Some(&publisher(BEN)),
            listing.id,
            InstallRequest::default(),
        )
        .await;
    let queue = service
        .pending_versions(&user(BEN), Some(&publisher(BEN)))
        .await
        .expect("a publisher lists what waits");

    approve(&service, listing.id, V1_0).await;
    let after = listed_for_cam(&service).await;
    let installed = service
        .install(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("anyone installs it once approved");

    assert!(before.is_empty());
    assert!(matches!(install_before, Err(MarketplaceError::NotFound)));
    assert_eq!(for_ben.len(), 1);
    assert_eq!(for_ben[0].pending_version, Some(V1_0));
    assert!(!for_ben[0].has_approved_version());
    assert!(matches!(
        install_by_publisher,
        Err(MarketplaceError::NotFound)
    ));
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].version, V1_0);
    assert_eq!(queue[0].approved_version, None);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].scope, ListingScope::Global);
    assert_eq!(after[0].latest_version, V1_0);
    assert_eq!(after[0].pending_version, None);
    assert_eq!(installed.installed_major, Some(1));
}

#[tokio::test]
async fn a_publisher_never_approves_their_own_version() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;

    let own = decide(&service, ADA, listing.id, V1_0, ReviewDecision::Approve).await;
    let other_version = decide(&service, BEN, listing.id, V1_1, ReviewDecision::Approve).await;
    approve(&service, listing.id, V1_0).await;
    let twice = decide(&service, BEN, listing.id, V1_0, ReviewDecision::Approve).await;

    assert!(matches!(own, Err(MarketplaceError::OwnVersion)));
    assert!(matches!(other_version, Err(MarketplaceError::NotFound)));
    assert!(matches!(twice, Err(MarketplaceError::NotFound)));
}

#[tokio::test]
async fn a_pending_page_version_reaches_installs_and_notifies_only_once_approved() {
    let (service, pages, _, notifier) = global_marketplace();
    let listing = global_board(&service, &pages).await;
    approve(&service, listing.id, V1_0).await;
    service
        .install(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("Cam installs 1.0");

    let published = publish_board_version(
        &service,
        &pages,
        listing.id,
        "<h1>Board, tidied</h1>",
        VersionBump::Minor,
    )
    .await;
    let while_pending = service
        .installed_build(&user(CAM), Some(&outsider()), listing.id)
        .await
        .expect("the install builds");
    let seen_while_pending = listed_for_cam(&service).await;
    let sent_while_pending = notifier.sent.lock().unwrap().len();

    approve(&service, listing.id, V1_1).await;
    let after = service
        .installed_build(&user(CAM), Some(&outsider()), listing.id)
        .await
        .expect("the install builds");

    assert_eq!(published.review, ReviewState::Pending);
    assert_eq!(published.listing.latest_version, V1_0);
    assert_eq!(published.listing.pending_version, Some(V1_1));
    assert_eq!(while_pending.version, V1_0);
    assert_eq!(while_pending.html, "<h1>Board</h1>");
    assert_eq!(seen_while_pending[0].latest_version, V1_0);
    assert_eq!(seen_while_pending[0].pending_version, None);
    assert_eq!(sent_while_pending, 0);
    assert_eq!(after.version, V1_1);
    assert_eq!(after.html, "<h1>Board, tidied</h1>");
    assert_eq!(
        notifier.sent.lock().unwrap().clone(),
        vec![(
            VersionNotice {
                listing_id: listing.id,
                version: V1_1,
            },
            vec![CAM.to_owned()],
        )]
    );
}

#[tokio::test]
async fn a_pending_major_version_installs_only_once_approved() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;
    approve(&service, listing.id, V1_0).await;
    publish_board_version(
        &service,
        &pages,
        listing.id,
        "<h1>Board 2</h1>",
        VersionBump::Major,
    )
    .await;

    let installed = service
        .install(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("Cam installs");
    approve(&service, listing.id, Version { major: 2, minor: 0 }).await;
    let after = listed_for_cam(&service).await;

    assert_eq!(installed.installed_major, Some(1));
    assert_eq!(after[0].latest_version, Version { major: 2, minor: 0 });
}

#[tokio::test]
async fn one_version_waits_at_a_time() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;
    pages.save(BOARD, "<h1>Board, tidied</h1>");

    let while_new = service
        .publish_version(
            &user(ADA),
            Some(&publisher(ADA)),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
        )
        .await;

    assert!(matches!(while_new, Err(MarketplaceError::ReviewPending)));
}

#[tokio::test]
async fn a_publisher_who_left_the_team_publishes_no_global_version() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;
    approve(&service, listing.id, V1_0).await;
    pages.save(BOARD, "<h1>Board, tidied</h1>");

    let refused = service
        .publish_version(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
        )
        .await;

    assert!(matches!(refused, Err(MarketplaceError::NotGlobalPublisher)));
}

#[tokio::test]
async fn rejecting_a_new_listing_unlists_it_so_its_source_publishes_again() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;

    let rejected = decide(
        &service,
        BEN,
        listing.id,
        V1_0,
        ReviewDecision::Reject {
            reason: Some("  It shows internal data.  ".to_owned()),
        },
    )
    .await
    .expect("another publisher rejects it");
    let queue = service
        .pending_versions(&user(BEN), Some(&publisher(BEN)))
        .await
        .expect("a publisher lists what waits");
    let again = service
        .publish(
            &user(ADA),
            Some(&publisher(ADA)),
            owned(ADA, BOARD),
            text("Board"),
            PublishScope::Global,
        )
        .await;

    assert_eq!(rejected.state, ReviewState::Rejected);
    assert!(queue.is_empty());
    assert!(listed_for_cam(&service).await.is_empty());
    assert!(again.is_ok_and(|listing| listing.pending_version == Some(V1_0)));
}

#[tokio::test]
async fn a_rejected_version_keeps_installs_on_the_approved_one_and_frees_its_number() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;
    approve(&service, listing.id, V1_0).await;
    publish_board_version(
        &service,
        &pages,
        listing.id,
        "<h1>Broken idea</h1>",
        VersionBump::Minor,
    )
    .await;

    decide(
        &service,
        BEN,
        listing.id,
        V1_1,
        ReviewDecision::Reject { reason: None },
    )
    .await
    .expect("another publisher rejects it");
    let after_reject = listed_for_cam(&service).await;
    let next = publish_board_version(
        &service,
        &pages,
        listing.id,
        "<h1>Better idea</h1>",
        VersionBump::Minor,
    )
    .await;

    assert_eq!(after_reject[0].latest_version, V1_0);
    assert_eq!(next.listing.pending_version, Some(V1_1));
}

#[tokio::test]
async fn a_rejection_reason_is_at_most_a_thousand_characters() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;

    let refused = decide(
        &service,
        BEN,
        listing.id,
        V1_0,
        ReviewDecision::Reject {
            reason: Some("x".repeat(1001)),
        },
    )
    .await;

    assert!(matches!(refused, Err(MarketplaceError::Invalid(_))));
}

#[tokio::test]
async fn a_page_preview_holds_its_build_and_source() {
    let (service, pages, _, _) = global_marketplace();
    let listing = global_board(&service, &pages).await;

    let preview = service
        .review_preview(&user(BEN), Some(&publisher(BEN)), listing.id)
        .await
        .expect("a publisher previews it");

    let ReviewPreview::Page {
        pending,
        build,
        html,
    } = preview
    else {
        panic!("a page previews as a page");
    };
    assert_eq!(pending.version, V1_0);
    assert_eq!(html, "<h1>Board</h1>");
    assert_eq!(build.source_hash, SourceHash::of("<h1>Board</h1>"));
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

fn digest_source(variables: Vec<TemplateVariable>) -> PublishSource {
    PublishSource::Routine {
        routine: EntityAccessReceipt::dangerously_assert_authenticated_user(
            user(ADA),
            &DIGEST.to_string(),
            EntityType::ScheduledAction,
        ),
        variables,
    }
}

fn zone() -> TemplateVariable {
    TemplateVariable::TimeZone {
        timezone: Tz::Europe__London,
        fill: VariableFill::Activator,
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

async fn publish_digest_version(
    service: &Service,
    routines: &FakeRoutines,
    listing: ListingId,
    instructions: &str,
    variables: Vec<TemplateVariable>,
) -> PublishedVersion {
    routines.save_source(ADA, DIGEST, digest(instructions));
    service
        .publish_version(
            &user(ADA),
            Some(&publisher(ADA)),
            listing,
            digest_source(variables),
            VersionBump::Minor,
        )
        .await
        .expect("the version publishes")
}

/// Ada's digest, published to everyone and approved, installed by Cam.
async fn installed_global_digest(service: &Service, routines: &FakeRoutines) -> (Listing, Uuid) {
    routines.save_source(ADA, DIGEST, digest("Sum up the week"));
    let listing = service
        .publish(
            &user(ADA),
            Some(&publisher(ADA)),
            digest_source(vec![zone()]),
            text("Weekly digest"),
            PublishScope::Global,
        )
        .await
        .expect("the digest publishes");
    approve(service, listing.id, V1_0).await;
    let installed = service
        .install(
            &user(CAM),
            Some(&outsider()),
            listing.id,
            InstallRequest {
                timezone: Some(Tz::America__New_York),
                ..InstallRequest::default()
            },
        )
        .await
        .expect("Cam installs it");
    let routine_id = installed
        .installed_routine
        .expect("an installed routine")
        .routine_id;
    (listing, routine_id)
}

#[tokio::test]
async fn a_pending_routine_minor_updates_installs_only_once_approved() {
    let (service, _, routines, _) = global_marketplace();
    let (listing, routine_id) = installed_global_digest(&service, &routines).await;

    let published = publish_digest_version(
        &service,
        &routines,
        listing.id,
        "Sum up the week briefly",
        vec![zone()],
    )
    .await;
    let while_pending = routines.saved(routine_id);
    let newest_while_pending = service
        .routine_version(&user(CAM), Some(&outsider()), listing.id)
        .await
        .expect("Cam reads the routine");
    let approved = approve(&service, listing.id, V1_1).await;
    let after = routines.saved(routine_id);

    assert_eq!(published.bump, VersionBump::Minor);
    assert_eq!(published.updates.updated, 0);
    assert_eq!(
        while_pending.routine.task["steps"][0]["instructions"],
        "Sum up the week"
    );
    assert_eq!(newest_while_pending.version, V1_0);
    assert_eq!(approved.updates.updated, 1);
    assert_eq!(
        after.routine.task["steps"][0]["instructions"],
        "Sum up the week briefly"
    );
}

#[tokio::test]
async fn the_forced_major_compares_with_the_newest_approved_version() {
    let (service, _, routines, _) = global_marketplace();
    let (listing, _) = installed_global_digest(&service, &routines).await;

    // A new variable asks something new of installs: a major, rejected.
    let forced = publish_digest_version(
        &service,
        &routines,
        listing.id,
        "Sum up the board",
        vec![zone(), board(VariableFill::Choice)],
    )
    .await;
    decide(
        &service,
        BEN,
        listing.id,
        forced.listing.pending_version.expect("a pending version"),
        ReviewDecision::Reject { reason: None },
    )
    .await
    .expect("another publisher rejects it");
    // Back to the approved interface: a minor again, numbered after 1.0.
    let minor = publish_digest_version(
        &service,
        &routines,
        listing.id,
        "Sum up the week, briefly",
        vec![zone()],
    )
    .await;

    assert_eq!(forced.bump, VersionBump::Major);
    assert_eq!(
        forced.listing.pending_version,
        Some(Version { major: 2, minor: 0 })
    );
    assert_eq!(minor.bump, VersionBump::Minor);
    assert_eq!(minor.listing.pending_version, Some(V1_1));
}

#[tokio::test]
async fn a_routine_preview_shows_its_interface_changes() {
    let (service, _, routines, _) = global_marketplace();
    let (listing, _) = installed_global_digest(&service, &routines).await;
    publish_digest_version(
        &service,
        &routines,
        listing.id,
        "Sum up the board",
        vec![zone(), board(VariableFill::Choice)],
    )
    .await;

    let preview = service
        .review_preview(&user(BEN), Some(&publisher(BEN)), listing.id)
        .await
        .expect("a publisher previews it");

    let ReviewPreview::Routine {
        pending,
        snapshot,
        interface_changes,
    } = preview
    else {
        panic!("a routine previews as a routine");
    };
    assert_eq!(pending.approved_version, Some(V1_0));
    assert_eq!(
        snapshot.task["steps"][0]["instructions"],
        "Sum up the board"
    );
    assert_eq!(
        interface_changes,
        vec![InterfaceChange {
            part: InterfacePart::Variable,
            change: InterfaceChangeKind::Added,
            name: "board (item)".to_owned(),
        }]
    );
}
