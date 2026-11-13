//! Repository behavior against Postgres.

use agent_code_mode::domain::SourceHash;
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use macro_user_id::user_id::MacroUserIdStr;
use sqlx::PgPool;
use uuid::Uuid;

use super::PgListingRepo;
use crate::domain::models::{
    AddedVersion, Listing, ListingCursor, ListingId, ListingKind, ListingQuery, ListingScope,
    ListingText, ListingsShown, NewListing, PublishedPage, Version, VersionBump,
};
use crate::domain::ports::ListingRepo;

const ADA: &str = "macro|ada@example.com";
const BEN: &str = "macro|ben@example.com";
const OTHER_OWNER: &str = "macro|olga@example.com";
const CARA: &str = "macro|cara@example.com";

fn user(id: &'static str) -> MacroUserIdStr<'static> {
    MacroUserIdStr::parse_from_str(id).expect("valid user id")
}

fn page(html: &str) -> PublishedPage {
    PublishedPage {
        source_hash: SourceHash::of(html),
        html: html.to_owned(),
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
    ListingQuery::new(shown, None, None, None, None).expect("a valid query")
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

/// Ada's board, published to `team` from a new document.
fn board(team: Uuid) -> NewListing {
    NewListing {
        id: ListingId::generate(),
        kind: ListingKind::Artifact,
        publisher: user(ADA),
        team_id: team,
        source_document_id: Uuid::now_v7(),
        text: ListingText {
            title: "Board".to_owned(),
            description: "Tasks by status".to_owned(),
        },
        page: page("<h1>Board</h1>"),
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
        .add_version(listing.id, VersionBump::Minor, &page("<h1>Board</h1>"))
        .await
        .unwrap();
    let minor = repo
        .add_version(listing.id, VersionBump::Minor, &page("<h1>Board 2</h1>"))
        .await
        .unwrap();
    let major = repo
        .add_version(listing.id, VersionBump::Major, &page("<h1>Board 3</h1>"))
        .await
        .unwrap();
    let after_major = repo
        .add_version(listing.id, VersionBump::Minor, &page("<h1>Board 4</h1>"))
        .await
        .unwrap();
    let missing = repo
        .add_version(
            ListingId::generate(),
            VersionBump::Minor,
            &page("<h1>Nothing</h1>"),
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
    repo.add_version(listing.id, VersionBump::Minor, &page("<h1>Board 1.1</h1>"))
        .await
        .unwrap();
    repo.add_version(listing.id, VersionBump::Major, &page("<h1>Board 2.0</h1>"))
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
    assert_eq!(waiting[0].latest_version, Version { major: 2, minor: 0 });
    assert_eq!(on_second_major.version, Version { major: 2, minor: 0 });
    assert_eq!(on_second_major.html, "<h1>Board 2.0</h1>");
    assert_eq!(moved[0].installed_major, Some(2));
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
        source_document_id: Uuid::now_v7(),
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
        source_document_id: Uuid::now_v7(),
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
        ListingQuery::new(ListingsShown::Listed, Some(text), None, None, None)
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
            source_document_id: Uuid::now_v7(),
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
async fn a_document_finds_its_live_listing(pool: PgPool) {
    let acme = insert_team(&pool, ADA).await;
    let repo = PgListingRepo::new(pool);
    let first = board(acme);
    let other = NewListing {
        id: ListingId::generate(),
        source_document_id: Uuid::now_v7(),
        ..board(acme)
    };
    repo.create_listing(first.clone()).await.unwrap();
    repo.create_listing(other).await.unwrap();
    repo.remove_listing(first.id).await.unwrap();
    let relisted = NewListing {
        id: ListingId::generate(),
        ..first.clone()
    };
    repo.create_listing(relisted.clone()).await.unwrap();
    let query = ListingQuery::new(
        ListingsShown::Listed,
        None,
        Some(first.source_document_id),
        None,
        None,
    )
    .expect("a valid query");

    let found = repo.listings(&user(ADA), Some(acme), &query).await.unwrap();

    assert_eq!(
        found
            .listings
            .iter()
            .map(|listing| listing.id)
            .collect::<Vec<_>>(),
        vec![relisted.id]
    );
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
