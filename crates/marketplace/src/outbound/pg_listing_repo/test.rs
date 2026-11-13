//! Repository behavior against Postgres.

use agent_code_mode::domain::SourceHash;
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use macro_user_id::user_id::MacroUserIdStr;
use sqlx::PgPool;
use uuid::Uuid;

use super::PgListingRepo;
use crate::domain::models::{
    AddedVersion, ListingId, ListingKind, ListingScope, ListingText, NewListing, PublishedPage,
    Version, VersionBump,
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
    let for_team = repo.listings(&user(BEN), Some(acme), None).await.unwrap();
    let for_other_team = repo.listings(&user(BEN), Some(other), None).await.unwrap();
    let without_team = repo.listings(&user(BEN), None, None).await.unwrap();

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
    let listed = repo.listings(&user(BEN), Some(acme), None).await.unwrap();

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
    let listed = repo.listings(&user(BEN), Some(acme), None).await.unwrap();

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
    let waiting = repo.listings(&user(BEN), Some(acme), None).await.unwrap();

    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    repo.install_latest(listing.id, &user(BEN)).await.unwrap();
    let on_second_major = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap()
        .expect("the install has a page");
    let moved = repo.listings(&user(BEN), Some(acme), None).await.unwrap();

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

    repo.remove_listing(listing.id).await.unwrap();
    let for_installer = repo.listings(&user(BEN), Some(acme), None).await.unwrap();
    let for_others = repo.listings(&user(ADA), Some(acme), None).await.unwrap();
    let page = repo
        .installed_page(listing.id, &user(BEN), Some(acme))
        .await
        .unwrap();

    repo.uninstall(listing.id, &user(BEN)).await.unwrap();
    repo.uninstall(listing.id, &user(BEN)).await.unwrap();
    let after_uninstall = repo.listings(&user(BEN), Some(acme), None).await.unwrap();

    assert_eq!(for_installer.len(), 1);
    assert!(for_installer[0].removed);
    assert!(for_others.is_empty());
    assert!(page.is_some());
    assert!(after_uninstall.is_empty());
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

    let without_team = repo.listings(&user(BEN), None, None).await.unwrap();
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

    let listed = repo.listings(&user(CARA), Some(acme), None).await.unwrap();
    let installs = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM marketplace_install"#)
        .fetch_one(&pool)
        .await
        .expect("installs should count");

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, adas.id);
    assert_eq!(installs, 0);
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
    repo.add_version(listing.id, VersionBump::Major, &page("<h1>Board 2.0</h1>"))
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
