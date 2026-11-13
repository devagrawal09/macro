//! Repository behavior against Postgres.

use std::collections::BTreeMap;

use agent_code_mode::domain::SourceHash;
use chrono_tz::Tz;
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use macro_user_id::user_id::MacroUserIdStr;
use scheduled_action::domain::templates::{
    RoutineSnapshot, TemplateVariable, VariableFill, fill::RoutineFills,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::PgListingRepo;
use crate::domain::models::{
    AddedVersion, Changelog, FollowingInstall, InstallUpdate, InstalledRoutine, Listing,
    ListingContent, ListingCursor, ListingId, ListingKind, ListingQuery, ListingScope, ListingText,
    ListingsShown, NewListing, PublishedPage, PublishedRoutine, RoutineInstall, TaskHash, Version,
    VersionBump, VersionNotes,
};
use crate::domain::ports::ListingRepo;

const ADA: &str = "macro|ada@example.com";
const BEN: &str = "macro|ben@example.com";
const OTHER_OWNER: &str = "macro|olga@example.com";
const CARA: &str = "macro|cara@example.com";

fn user(id: &'static str) -> MacroUserIdStr<'static> {
    MacroUserIdStr::parse_from_str(id).expect("valid user id")
}

fn page(html: &str) -> ListingContent {
    ListingContent::Page(PublishedPage {
        source_hash: SourceHash::of(html),
        html: html.to_owned(),
    })
}

fn routine(name: &str) -> ListingContent {
    ListingContent::Routine(PublishedRoutine::new(snapshot(name)))
}

fn snapshot(name: &str) -> RoutineSnapshot {
    RoutineSnapshot {
        name: name.to_owned(),
        description: "Sums up the week".to_owned(),
        trigger: serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "Europe/London",
        }))
        .unwrap(),
        task: json!({ "prompt": "", "user_prompt": "Sum up the week" }),
        variables: vec![TemplateVariable::TimeZone {
            timezone: Tz::Europe__London,
            fill: VariableFill::Activator,
        }],
    }
}

fn install(minor: i32, routine_id: Uuid, task_hash: &str) -> RoutineInstall {
    RoutineInstall {
        version: Version { major: 1, minor },
        routine_id,
        fills: RoutineFills {
            timezone: Tz::America__New_York,
            variables: BTreeMap::from([("board".to_owned(), VariableFill::Choice)]),
        },
        task_hash: TaskHash(task_hash.to_owned()),
    }
}

/// A new user `id`, as installs and listings reference them.
async fn insert_user(pool: &PgPool, id: &str) {
    let macro_user_id = Uuid::now_v7();
    sqlx::query!(
        r#"INSERT INTO macro_user (id, username, email, stripe_customer_id) VALUES ($1, $2, $2, $2)"#,
        macro_user_id,
        id,
    )
    .execute(pool)
    .await
    .expect("macro_user should insert");
    sqlx::query!(
        r#"INSERT INTO "User" (id, email, macro_user_id) VALUES ($1, $1, $2)"#,
        id,
        macro_user_id,
    )
    .execute(pool)
    .await
    .expect("user should insert");
}

/// The first page of `shown`, of the default size.
fn first_page(shown: ListingsShown) -> ListingQuery {
    ListingQuery::new(shown, None, None, None, None, None).expect("a valid query")
}

/// What Browse lists for `user`, as a member of `team`.
async fn browse(repo: &PgListingRepo, user_id: &'static str, team: Option<Uuid>) -> Vec<Listing> {
    repo.listings(&user(user_id), team, &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings
}

/// A team owned by a new user `owner`; a person owns one team at most.
async fn insert_team(pool: &PgPool, owner: &str) -> Uuid {
    insert_user(pool, owner).await;

    let team = Uuid::now_v7();
    sqlx::query!(
        r#"INSERT INTO team (id, name, owner_id) VALUES ($1, $2, $2)"#,
        team,
        owner,
    )
    .execute(pool)
    .await
    .expect("team should insert");
    team
}

/// Make `user` a member of `team`.
async fn join_team(pool: &PgPool, team: Uuid, user: &str) {
    sqlx::query!(
        r#"INSERT INTO team_user (user_id, team_id, team_role) VALUES ($1, $2, 'member')"#,
        user,
        team,
    )
    .execute(pool)
    .await
    .expect("team member should insert");
}

/// Ada's board, published to `team` from a new document.
fn board(team: Uuid) -> NewListing {
    NewListing {
        id: ListingId::generate(),
        publisher: user(ADA),
        team_id: team,
        source_id: Uuid::now_v7(),
        text: ListingText {
            title: "Board".to_owned(),
            description: "Tasks by status".to_owned(),
        },
        content: page("<h1>Board</h1>"),
        changelog: Changelog::default(),
    }
}

/// Ada's weekly digest, published to `team` from a new routine.
fn digest(team: Uuid) -> NewListing {
    NewListing {
        id: ListingId::generate(),
        publisher: user(ADA),
        team_id: team,
        source_id: Uuid::now_v7(),
        text: ListingText {
            title: "Weekly digest".to_owned(),
            description: "Sums up the week".to_owned(),
        },
        content: routine("Weekly digest"),
        changelog: Changelog::default(),
    }
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_listing_is_seen_by_its_team_with_its_first_version(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let other = insert_team(&pool, OTHER_OWNER).await;
    let repo = PgListingRepo::new(pool);
    let listing = board(acme);

    let created = repo.create_listing(listing.clone()).await.unwrap();
    let for_team = browse(&repo, BEN, Some(acme)).await;
    let for_other_team = browse(&repo, BEN, Some(other)).await;
    let without_team = browse(&repo, BEN, None).await;

    assert!(created);
    assert_eq!(for_team.len(), 1);
    assert_eq!(for_team[0].id, listing.id);
    assert_eq!(for_team[0].scope, ListingScope::Team { team_id: acme });
    assert_eq!(for_team[0].publisher, user(ADA));
    assert_eq!(for_team[0].title, "Board");
    assert_eq!(for_team[0].latest_version, Version { major: 1, minor: 0 });
    assert_eq!(for_team[0].kind, ListingKind::Artifact);
    assert_eq!(for_team[0].installed_major, None);
    assert_eq!(for_team[0].install_count, 0);
    assert!(for_other_team.is_empty());
    assert!(without_team.is_empty());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_document_lists_once_until_its_listing_is_removed(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let first = board(acme);
    let second = NewListing {
        id: ListingId::generate(),
        ..first.clone()
    };
    let third = NewListing {
        id: ListingId::generate(),
        ..first.clone()
    };

    let created = repo.create_listing(first.clone()).await.unwrap();
    let repeated = repo.create_listing(second).await.unwrap();
    repo.remove_listing(first.id).await.unwrap();
    let relisted = repo.create_listing(third.clone()).await.unwrap();
    let listed = browse(&repo, BEN, Some(acme)).await;

    assert!(created);
    assert!(!repeated);
    assert!(relisted);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, third.id);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn versions_number_from_the_newest_and_skip_an_unchanged_page(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let listing = board(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    let unchanged = repo
        .add_version(
            listing.id,
            VersionBump::Minor,
            &page("<h1>Board</h1>"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    let minor = repo
        .add_version(
            listing.id,
            VersionBump::Minor,
            &page("<h1>Board 2</h1>"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    let major = repo
        .add_version(
            listing.id,
            VersionBump::Major,
            &page("<h1>Board 3</h1>"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    let after_major = repo
        .add_version(
            listing.id,
            VersionBump::Minor,
            &page("<h1>Board 4</h1>"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    let missing = repo
        .add_version(
            ListingId::generate(),
            VersionBump::Minor,
            &page("<h1>Nothing</h1>"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    let listed = browse(&repo, BEN, Some(acme)).await;

    assert_eq!(unchanged, AddedVersion::Unchanged);
    assert_eq!(minor, AddedVersion::Added(Version { major: 1, minor: 1 }));
    assert_eq!(major, AddedVersion::Added(Version { major: 2, minor: 0 }));
    assert_eq!(
        after_major,
        AddedVersion::Added(Version { major: 2, minor: 1 })
    );
    assert_eq!(missing, AddedVersion::Missing);
    assert_eq!(listed[0].latest_version, Version { major: 2, minor: 1 });
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_history_keeps_each_versions_changelog_newest_first(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let listing = NewListing {
        changelog: Changelog::new(Some("First cut")).unwrap(),
        ..digest(acme)
    };
    repo.create_listing(listing.clone()).await.unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Major,
        &routine("Weekly digest, with a board"),
        &VersionNotes {
            changelog: Changelog::new(Some("Asks for a board")).unwrap(),
            forced_major: true,
        },
    )
    .await
    .unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Minor,
        &routine("Weekly digest, tidied"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();

    let history = repo.versions(listing.id).await.unwrap();
    let unknown = repo.versions(ListingId::generate()).await.unwrap();

    let summary: Vec<(Version, Option<&str>, bool)> = history
        .iter()
        .map(|entry| {
            (
                entry.version,
                entry.changelog.as_deref(),
                entry.forced_major,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (Version { major: 2, minor: 1 }, None, false),
            (
                Version { major: 2, minor: 0 },
                Some("Asks for a board"),
                true
            ),
            (Version::FIRST, Some("First cut"), false),
        ]
    );
    assert!(unknown.is_empty());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_install_shows_the_newest_minor_of_its_major_until_it_moves(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let other = insert_team(&pool, OTHER_OWNER).await;
    let repo = PgListingRepo::new(pool);
    let listing = board(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    let before = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap();
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Minor,
        &page("<h1>Board 1.1</h1>"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Major,
        &page("<h1>Board 2.0</h1>"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    let on_first_major = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap()
        .expect("the install has a page");
    let from_another_team = repo
        .installed_page(listing.id, &user(BEN), Some(other))
        .await
        .unwrap();
    let waiting = browse(&repo, BEN, Some(acme)).await;

    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    let on_second_major = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap()
        .expect("the install has a page");
    let moved = browse(&repo, BEN, Some(acme)).await;

    assert_eq!(before, None);
    assert_eq!(on_first_major.version, Version { major: 1, minor: 1 });
    assert_eq!(on_first_major.html, "<h1>Board 1.1</h1>");
    assert_eq!(from_another_team, None);
    assert_eq!(waiting[0].installed_major, Some(1));
    assert_eq!(
        waiting[0].installed_version,
        Some(Version { major: 1, minor: 1 })
    );
    assert_eq!(waiting[0].latest_version, Version { major: 2, minor: 0 });
    assert_eq!(on_second_major.version, Version { major: 2, minor: 0 });
    assert_eq!(on_second_major.html, "<h1>Board 2.0</h1>");
    assert_eq!(moved[0].installed_major, Some(2));
    assert_eq!(
        moved[0].installed_version,
        Some(Version { major: 2, minor: 0 })
    );
    assert_eq!(moved[0].install_count, 1);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_removed_listing_stays_only_for_those_who_installed_it(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool);
    let listing = board(acme);
    repo.create_listing(listing.clone()).await.unwrap();
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    let installed = first_page(ListingsShown::Installed);

    repo.remove_listing(listing.id).await.unwrap();
    let browsed_by_installer = browse(&repo, BEN, Some(acme)).await;
    let installed_by_installer = repo
        .listings(&user(BEN), Some(acme), &installed)
        .await
        .unwrap()
        .listings;
    let looked_up_by_installer = repo
        .listing(&user(BEN), Some(acme), listing.id)
        .await
        .unwrap();
    let looked_up_by_others = repo
        .listing(&user(ADA), Some(acme), listing.id)
        .await
        .unwrap();
    let page = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap();

    repo.uninstall(listing.id, &user(BEN)).await.unwrap();
    repo.uninstall(listing.id, &user(BEN)).await.unwrap();
    let after_uninstall = repo
        .listings(&user(BEN), Some(acme), &installed)
        .await
        .unwrap()
        .listings;

    assert!(browsed_by_installer.is_empty());
    assert_eq!(installed_by_installer.len(), 1);
    assert!(installed_by_installer[0].removed);
    assert!(looked_up_by_installer.is_some_and(|listing| listing.removed));
    assert_eq!(looked_up_by_others, None);
    assert!(page.is_some());
    assert!(after_uninstall.is_empty());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn installed_lists_only_the_callers_installs(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool);
    let board = board(acme);
    let notes = NewListing {
        id: ListingId::generate(),
        source_id: Uuid::now_v7(),
        ..board.clone()
    };
    repo.create_listing(board.clone()).await.unwrap();
    repo.create_listing(notes.clone()).await.unwrap();
    repo.install_latest(notes.id, &user(BEN)).await.unwrap();
    let installed = first_page(ListingsShown::Installed);

    let bens = repo
        .listings(&user(BEN), Some(acme), &installed)
        .await
        .unwrap();
    let adas = repo
        .listings(&user(ADA), Some(acme), &installed)
        .await
        .unwrap();

    assert_eq!(
        bens.listings
            .iter()
            .map(|listing| listing.id)
            .collect::<Vec<_>>(),
        vec![notes.id]
    );
    assert_eq!(bens.next, None);
    assert!(adas.listings.is_empty());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_search_matches_the_title_or_description_literally(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let listing = |title: &str, description: &str| NewListing {
        id: ListingId::generate(),
        source_id: Uuid::now_v7(),
        text: ListingText {
            title: title.to_owned(),
            description: description.to_owned(),
        },
        ..board(acme)
    };
    let launch = listing("Launch Dashboard", "Tracks the launch");
    let notes = listing("Release notes", "What shipped, by DASHBOARD area");
    let discount = listing("Pricing", "100% off_peak");
    let other = listing("Pricing table", "100 percent of peak");
    for listing in [&launch, &notes, &discount, &other] {
        repo.create_listing(listing.clone()).await.unwrap();
    }
    let search = |text: &str| {
        ListingQuery::new(ListingsShown::Listed, Some(text), None, None, None, None)
            .expect("a valid query")
    };
    let ids = |listings: Vec<Listing>| {
        let mut ids: Vec<_> = listings.into_iter().map(|listing| listing.id).collect();
        ids.sort_by_key(|id| id.as_uuid());
        ids
    };

    let dashboards = repo
        .listings(&user(BEN), Some(acme), &search("dashboard"))
        .await
        .unwrap();
    let wildcards = repo
        .listings(&user(BEN), Some(acme), &search("0% off_"))
        .await
        .unwrap();
    let nothing = repo
        .listings(&user(BEN), Some(acme), &search("roadmap"))
        .await
        .unwrap();

    let mut expected = vec![launch.id, notes.id];
    expected.sort_by_key(|id| id.as_uuid());
    assert_eq!(ids(dashboards.listings), expected);
    assert_eq!(ids(wildcards.listings), vec![discount.id]);
    assert!(nothing.listings.is_empty());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn pages_follow_newest_first_and_end_on_the_last_listing(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool.clone());
    // Five listings, the last two created at the same instant, so the
    // cursor's id orders them.
    let mut listings = Vec::new();
    for _ in 0..5 {
        let listing = NewListing {
            id: ListingId::generate(),
            source_id: Uuid::now_v7(),
            ..board(acme)
        };
        repo.create_listing(listing.clone()).await.unwrap();
        listings.push(listing);
    }
    for (offset, listing) in [4, 3, 2, 1, 1].into_iter().zip(&listings) {
        sqlx::query!(
            r#"
            UPDATE marketplace_listing
            SET created_at = '2026-11-01T00:00:00Z'::timestamptz + make_interval(mins => $2)
            WHERE id = $1
            "#,
            listing.id.as_uuid(),
            offset,
        )
        .execute(&pool)
        .await
        .expect("listing should update");
    }
    let mut tied = [listings[3].id, listings[4].id];
    tied.sort_by_key(|id| id.as_uuid());
    let newest_first = vec![
        listings[0].id,
        listings[1].id,
        listings[2].id,
        tied[0],
        tied[1],
    ];
    let page = |cursor: Option<String>, limit: u16| {
        ListingQuery::new(
            ListingsShown::Listed,
            None,
            None,
            None,
            cursor.as_deref(),
            Some(limit),
        )
        .expect("a valid query")
    };

    let mut seen = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let read = repo
            .listings(&user(BEN), Some(acme), &page(cursor, 2))
            .await
            .unwrap();
        pages += 1;
        seen.extend(read.listings.iter().map(|listing| listing.id));
        let Some(next) = read.next else { break };
        cursor = Some(next.encode());
    }
    let exact = repo
        .listings(&user(BEN), Some(acme), &page(None, 5))
        .await
        .unwrap();
    let after_fourth = repo
        .listings(
            &user(BEN),
            Some(acme),
            &page(Some(ListingCursor::after(&exact.listings[3]).encode()), 2),
        )
        .await
        .unwrap();

    assert_eq!(seen, newest_first);
    assert_eq!(pages, 3);
    assert_eq!(exact.listings.len(), 5);
    assert_eq!(exact.next, None);
    assert_eq!(
        after_fourth
            .listings
            .iter()
            .map(|listing| listing.id)
            .collect::<Vec<_>>(),
        vec![tied[1]]
    );
    assert_eq!(after_fourth.next, None);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_source_finds_its_live_listing_of_its_kind(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let first = board(acme);
    let other = NewListing {
        id: ListingId::generate(),
        source_id: Uuid::now_v7(),
        ..board(acme)
    };
    // A routine with the document's id lists apart from it.
    let routine = NewListing {
        source_id: first.source_id,
        ..digest(acme)
    };
    repo.create_listing(first.clone()).await.unwrap();
    repo.create_listing(other).await.unwrap();
    repo.create_listing(routine.clone()).await.unwrap();
    repo.remove_listing(first.id).await.unwrap();
    let relisted = NewListing {
        id: ListingId::generate(),
        ..first.clone()
    };
    repo.create_listing(relisted.clone()).await.unwrap();
    let source = |kind| {
        ListingQuery::new(
            ListingsShown::Listed,
            None,
            Some(kind),
            Some(first.source_id),
            None,
            None,
        )
        .expect("a valid query")
    };
    let ids = |listings: Vec<Listing>| {
        listings
            .into_iter()
            .map(|listing| listing.id)
            .collect::<Vec<_>>()
    };

    let artifact = repo
        .listings(&user(ADA), Some(acme), &source(ListingKind::Artifact))
        .await
        .unwrap();
    let routines = repo
        .listings(&user(ADA), Some(acme), &source(ListingKind::Routine))
        .await
        .unwrap();

    assert_eq!(ids(artifact.listings), vec![relisted.id]);
    assert_eq!(ids(routines.listings), vec![routine.id]);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_listing_without_a_team_is_seen_by_everyone(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool.clone());
    let listing = board(acme);
    repo.create_listing(listing.clone()).await.unwrap();
    sqlx::query!(
        r#"UPDATE marketplace_listing SET team_id = NULL WHERE id = $1"#,
        listing.id.as_uuid(),
    )
    .execute(&pool)
    .await
    .expect("listing should update");

    let without_team = browse(&repo, BEN, None).await;
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    let page = repo
        .installed_page(listing.id, &user(BEN), None)
        .await
        .unwrap();

    assert_eq!(without_team.len(), 1);
    assert_eq!(without_team[0].scope, ListingScope::Global);
    assert!(page.is_some());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn deleting_a_user_removes_their_installs_and_listings(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    insert_user(&pool, CARA).await;
    let repo = PgListingRepo::new(pool.clone());
    let adas = board(acme);
    let bens = NewListing {
        publisher: user(BEN),
        ..board(acme)
    };
    repo.create_listing(adas.clone()).await.unwrap();
    repo.create_listing(bens.clone()).await.unwrap();
    repo.install_latest(adas.id, &user(BEN)).await.unwrap();
    repo.install_latest(bens.id, &user(CARA)).await.unwrap();

    sqlx::query!(r#"DELETE FROM "User" WHERE id = $1"#, BEN)
        .execute(&pool)
        .await
        .expect("user should delete");

    let listed = browse(&repo, CARA, Some(acme)).await;
    let installs = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM marketplace_install"#)
        .fetch_one(&pool)
        .await
        .expect("installs should count");

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, adas.id);
    assert_eq!(installs, 0);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_routine_and_a_document_with_the_same_id_list_apart(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let artifact = board(acme);
    let routine = NewListing {
        source_id: artifact.source_id,
        ..digest(acme)
    };

    let artifact_created = repo.create_listing(artifact).await.unwrap();
    let routine_created = repo.create_listing(routine.clone()).await.unwrap();
    let repeated = repo
        .create_listing(NewListing {
            id: ListingId::generate(),
            ..routine.clone()
        })
        .await
        .unwrap();

    assert!(artifact_created);
    assert!(routine_created);
    assert!(!repeated);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_routine_version_stores_its_snapshot_and_no_page(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool.clone());
    let listing = digest(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    let unchanged = repo
        .add_version(
            listing.id,
            VersionBump::Minor,
            &routine("Weekly digest"),
            &VersionNotes::default(),
        )
        .await
        .unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Minor,
        &routine("Weekly digest 1.1"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Major,
        &routine("Weekly digest 2"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    let newest = repo.routine_version(listing.id, None).await.unwrap();
    let first_major = repo.routine_version(listing.id, Some(1)).await.unwrap();
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    let page = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap();
    let wrong_column = sqlx::query!(
        r#"
        INSERT INTO marketplace_listing_version
            (listing_id, kind, major, minor, source_hash, html)
        VALUES ($1, 'routine', 9, 0, 'hash', '<h1>Not a routine</h1>')
        "#,
        listing.id.as_uuid(),
    )
    .execute(&pool)
    .await;
    let wrong_kind = sqlx::query!(
        r#"
        INSERT INTO marketplace_listing_version
            (listing_id, kind, major, minor, source_hash, html)
        VALUES ($1, 'artifact', 9, 0, 'hash', '<h1>Not a routine</h1>')
        "#,
        listing.id.as_uuid(),
    )
    .execute(&pool)
    .await;

    assert_eq!(unchanged, AddedVersion::Unchanged);
    let newest = newest.expect("the listing has a routine version");
    assert_eq!(newest.version, Version { major: 2, minor: 0 });
    assert_eq!(newest.snapshot, snapshot("Weekly digest 2"));
    let first_major = first_major.expect("the first major has a version");
    assert_eq!(first_major.version, Version { major: 1, minor: 1 });
    assert_eq!(first_major.snapshot.name, "Weekly digest 1.1");
    // A routine has no page to show.
    assert_eq!(page, None);
    assert!(wrong_column.is_err());
    assert!(wrong_kind.is_err());
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_routine_install_keeps_its_routine_fills_and_task_hash(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool);
    let listing = digest(acme);
    repo.create_listing(listing.clone()).await.unwrap();
    let routine_id = Uuid::now_v7();

    let before = repo.routine_install(listing.id, &user(BEN)).await.unwrap();
    repo.save_routine_install(listing.id, &user(BEN), &install(0, routine_id, "first"))
        .await
        .unwrap();
    let saved = repo.routine_install(listing.id, &user(BEN)).await.unwrap();
    let listed = repo
        .listings(&user(BEN), Some(acme), &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings;

    assert_eq!(before, None);
    assert_eq!(saved, Some(install(0, routine_id, "first")));
    assert_eq!(listed[0].installed_major, Some(1));
    assert_eq!(
        listed[0].installed_routine,
        Some(InstalledRoutine {
            routine_id,
            detached: false,
            update_failed: false,
        })
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn updates_reach_installs_behind_on_the_major_until_they_detach(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    insert_user(&pool, CARA).await;
    let repo = PgListingRepo::new(pool);
    let listing = digest(acme);
    repo.create_listing(listing.clone()).await.unwrap();
    let bens = Uuid::now_v7();
    let caras = Uuid::now_v7();
    repo.save_routine_install(listing.id, &user(BEN), &install(0, bens, "ben"))
        .await
        .unwrap();
    repo.save_routine_install(listing.id, &user(CARA), &install(0, caras, "cara"))
        .await
        .unwrap();
    let minor = Version { major: 1, minor: 1 };

    let behind = repo.installs_behind(listing.id, minor).await.unwrap();
    repo.record_update(
        listing.id,
        &user(BEN),
        &InstallUpdate::Updated {
            version: minor,
            task_hash: TaskHash("ben 1.1".to_owned()),
        },
    )
    .await
    .unwrap();
    repo.record_update(listing.id, &user(CARA), &InstallUpdate::Failed)
        .await
        .unwrap();
    let after_first = repo.installs_behind(listing.id, minor).await.unwrap();
    let cara_failed = repo
        .listings(&user(CARA), Some(acme), &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings;
    repo.record_update(listing.id, &user(CARA), &InstallUpdate::Detached)
        .await
        .unwrap();
    let after_detach = repo
        .installs_behind(listing.id, Version { major: 1, minor: 2 })
        .await
        .unwrap();
    let cara_detached = repo
        .listings(&user(CARA), Some(acme), &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings;
    let ben_updated = repo.routine_install(listing.id, &user(BEN)).await.unwrap();
    repo.save_routine_install(listing.id, &user(CARA), &install(1, caras, "cara again"))
        .await
        .unwrap();
    let cara_reinstalled = repo
        .listings(&user(CARA), Some(acme), &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings;

    let mut users: Vec<String> = behind
        .iter()
        .map(|FollowingInstall { user, .. }| user.as_ref().to_owned())
        .collect();
    users.sort();
    assert_eq!(users, vec![BEN.to_owned(), CARA.to_owned()]);
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].user, user(CARA));
    assert_eq!(after_first[0].install, install(0, caras, "cara"));
    assert!(
        cara_failed[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| routine.update_failed)
    );
    assert_eq!(after_detach.len(), 1);
    assert_eq!(after_detach[0].user, user(BEN));
    assert!(
        cara_detached[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| { routine.detached && !routine.update_failed })
    );
    assert_eq!(ben_updated, Some(install(1, bens, "ben 1.1")));
    assert!(
        cara_reinstalled[0]
            .installed_routine
            .as_ref()
            .is_some_and(|routine| !routine.detached)
    );
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn one_of_two_concurrent_first_installs_claims_the_routine(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    join_team(&pool, acme, BEN).await;
    let repo = PgListingRepo::new(pool);
    let listing = digest(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    let claim = |repo: PgListingRepo| {
        tokio::spawn(async move {
            repo.claim_routine_install(listing.id, &user(BEN), 1, 120)
                .await
                .unwrap()
        })
    };
    let (first, second) = tokio::join!(claim(repo.clone()), claim(repo.clone()));
    let while_claimed = repo
        .listings(&user(BEN), Some(acme), &first_page(ListingsShown::Listed))
        .await
        .unwrap()
        .listings;
    let installers_while_claimed = repo.installers(listing.id, None, 10).await.unwrap();
    let routine_id = Uuid::now_v7();
    repo.save_routine_install(listing.id, &user(BEN), &install(0, routine_id, "first"))
        .await
        .unwrap();
    let after_install = repo
        .claim_routine_install(listing.id, &user(BEN), 1, 0)
        .await
        .unwrap();
    repo.release_routine_claim(listing.id, &user(BEN))
        .await
        .unwrap();
    let kept = repo.routine_install(listing.id, &user(BEN)).await.unwrap();
    let installers = repo.installers(listing.id, None, 10).await.unwrap();

    // Exactly one claims it.
    assert_ne!(first.unwrap(), second.unwrap());
    // A claim still creating its routine isn't shown as an install.
    assert_eq!(while_claimed[0].installed_major, None);
    assert_eq!(while_claimed[0].install_count, 0);
    // Nor is it told of versions until its routine is saved.
    assert_eq!(installers_while_claimed, Vec::new());
    assert_eq!(installers, vec![user(BEN)]);
    // Neither claiming nor releasing touches an install with a routine.
    assert!(!after_install);
    assert_eq!(kept, Some(install(0, routine_id, "first")));
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_stale_or_released_claim_is_taken_by_the_next_install(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    let repo = PgListingRepo::new(pool.clone());
    let listing = digest(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    let claimed = repo
        .claim_routine_install(listing.id, &user(BEN), 1, 120)
        .await
        .unwrap();
    let while_fresh = repo
        .claim_routine_install(listing.id, &user(BEN), 1, 120)
        .await
        .unwrap();
    sqlx::query!(
        r#"
        UPDATE marketplace_install
        SET routine_claimed_at = now() - interval '10 minutes'
        WHERE listing_id = $1
        "#,
        listing.id.as_uuid(),
    )
    .execute(&pool)
    .await
    .unwrap();
    let taken_over = repo
        .claim_routine_install(listing.id, &user(BEN), 1, 120)
        .await
        .unwrap();
    repo.release_routine_claim(listing.id, &user(BEN))
        .await
        .unwrap();
    let after_release = repo
        .claim_routine_install(listing.id, &user(BEN), 1, 120)
        .await
        .unwrap();

    assert!(claimed);
    assert!(!while_fresh);
    assert!(taken_over);
    assert!(after_release);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn installers_are_team_members_on_the_major_newest_install_first(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    insert_user(&pool, CARA).await;
    join_team(&pool, acme, BEN).await;
    join_team(&pool, acme, CARA).await;
    let repo = PgListingRepo::new(pool);
    let listing = board(acme);
    repo.create_listing(listing.clone()).await.unwrap();

    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    repo.add_version(
        listing.id,
        VersionBump::Major,
        &page("<h1>Board 2.0</h1>"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    repo.install_latest(listing.id, &user(CARA)).await.unwrap();
    // Ada owns the team but has no member row: like someone who left, she
    // can't open the listing, so she isn't told of it.
    repo.install_latest(listing.id, &user(ADA)).await.unwrap();

    let on_second_major = repo.installers(listing.id, Some(2), 10).await.unwrap();
    let on_any_major = repo.installers(listing.id, None, 10).await.unwrap();
    let bounded = repo.installers(listing.id, None, 1).await.unwrap();

    assert_eq!(on_second_major, vec![user(CARA)]);
    assert_eq!(on_any_major, vec![user(CARA), user(BEN)]);
    assert_eq!(bounded, vec![user(CARA)]);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_pin_holds_a_page_on_its_minor_and_keeps_a_routine_from_updates(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    insert_user(&pool, BEN).await;
    insert_user(&pool, CARA).await;
    let repo = PgListingRepo::new(pool);
    let page_listing = board(acme);
    let routine_listing = digest(acme);
    repo.create_listing(page_listing.clone()).await.unwrap();
    repo.create_listing(routine_listing.clone()).await.unwrap();
    let not_installed = repo
        .set_pinned(page_listing.id, &user(BEN), true)
        .await
        .unwrap();

    // Ben pins his page on 1.0; a minor version follows.
    repo.install_latest(page_listing.id, &user(BEN))
        .await
        .unwrap();
    repo.set_pinned(page_listing.id, &user(BEN), true)
        .await
        .unwrap();
    repo.add_version(
        page_listing.id,
        VersionBump::Minor,
        &page("<h1>Board 1.1</h1>"),
        &VersionNotes::default(),
    )
    .await
    .unwrap();
    // Pinning again keeps the pin on 1.0.
    repo.set_pinned(page_listing.id, &user(BEN), true)
        .await
        .unwrap();
    let pinned_page = repo
        .installed_page(page_listing.id, &user(BEN), Some(acme))
        .await
        .unwrap()
        .expect("the install has a page");
    let pinned_listing = repo
        .listing(&user(BEN), Some(acme), page_listing.id)
        .await
        .unwrap()
        .expect("Ben sees the listing");
    repo.set_pinned(page_listing.id, &user(BEN), false)
        .await
        .unwrap();
    let unpinned_page = repo
        .installed_page(page_listing.id, &user(BEN), Some(acme))
        .await
        .unwrap()
        .expect("the install has a page");
    repo.set_pinned(page_listing.id, &user(BEN), true)
        .await
        .unwrap();
    repo.install_latest(page_listing.id, &user(BEN))
        .await
        .unwrap();
    let after_taking = repo
        .listing(&user(BEN), Some(acme), page_listing.id)
        .await
        .unwrap()
        .expect("Ben sees the listing");

    // Ben pins his routine; Cara doesn't, so only she is behind.
    repo.save_routine_install(
        routine_listing.id,
        &user(BEN),
        &install(0, Uuid::now_v7(), "ben"),
    )
    .await
    .unwrap();
    repo.save_routine_install(
        routine_listing.id,
        &user(CARA),
        &install(0, Uuid::now_v7(), "cara"),
    )
    .await
    .unwrap();
    repo.set_pinned(routine_listing.id, &user(BEN), true)
        .await
        .unwrap();
    let behind = repo
        .installs_behind(routine_listing.id, Version { major: 1, minor: 1 })
        .await
        .unwrap();
    let pinned_routine = repo
        .listing(&user(BEN), Some(acme), routine_listing.id)
        .await
        .unwrap()
        .expect("Ben sees the listing");

    assert!(!not_installed);
    assert_eq!(pinned_page.version, Version::FIRST);
    assert!(pinned_page.pinned);
    assert!(pinned_listing.pinned);
    assert_eq!(pinned_listing.installed_version, Some(Version::FIRST));
    assert_eq!(unpinned_page.version, Version { major: 1, minor: 1 });
    assert!(!unpinned_page.pinned);
    assert!(!after_taking.pinned, "taking a version unpins");
    let behind: Vec<String> = behind
        .into_iter()
        .map(|following| following.user.as_ref().to_owned())
        .collect();
    assert_eq!(behind, vec![CARA.to_owned()]);
    assert!(pinned_routine.pinned);
    assert_eq!(pinned_routine.installed_version, Some(Version::FIRST));
}
