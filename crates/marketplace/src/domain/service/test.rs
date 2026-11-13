//! Service-level rules, over hand-written fakes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_code_mode::domain::{
    BuildOutcome, DocumentBuild, DocumentProgramError, ProgramId, RunProgram, SourceHash,
    UserExecutionReceipt,
};
use chrono::Utc;
use entity_access::domain::models::{
    Entity, EntityAccessReceipt, EntityPermission, EntityType, MemberTeamRole, OwnerAccessLevel,
    TeamRole, ViewAccessLevel,
};
use macro_user_id::user_id::MacroUserIdStr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::MarketplaceServiceImpl;
use crate::domain::models::{
    AddedVersion, InstalledPage, Listing, ListingId, ListingPage, ListingQuery, ListingScope,
    ListingText, ListingsShown, MarketplaceError, NewListing, PublishedPage, Version, VersionBump,
};
use crate::domain::ports::{ListingRepo, MarketplaceService, Pages, Rollout};

const ADA: &str = "macro|ada@example.com";
const BEN: &str = "macro|ben@example.com";
const CAM: &str = "macro|cam@example.com";

const ACME: Uuid = Uuid::from_u128(1);
const OTHER_TEAM: Uuid = Uuid::from_u128(2);
const BOARD: Uuid = Uuid::from_u128(10);

fn user(id: &'static str) -> MacroUserIdStr<'static> {
    MacroUserIdStr::parse_from_str(id).expect("valid user id")
}

fn member(id: &'static str, team: Uuid, role: TeamRole) -> EntityAccessReceipt<MemberTeamRole> {
    EntityAccessReceipt::try_new_authenticated_user(
        user(id),
        Entity {
            entity_id: team.to_string(),
            entity_type: EntityType::Team,
        },
        EntityPermission::TeamRole { role },
    )
    .expect("a team role satisfies membership")
}

fn owned(id: &'static str, document: Uuid) -> EntityAccessReceipt<OwnerAccessLevel> {
    EntityAccessReceipt::dangerously_assert_authenticated_user(
        user(id),
        &document.to_string(),
        EntityType::Document,
    )
}

/// The first page of `shown`, as the web app asks for it.
fn first_page(shown: ListingsShown) -> ListingQuery {
    ListingQuery::new(shown, None, None, None, None).expect("a valid query")
}

fn text(title: &str) -> ListingText {
    ListingText {
        title: title.to_owned(),
        description: String::new(),
    }
}

struct Stored {
    listing: NewListing,
    removed: bool,
    versions: Vec<(Version, PublishedPage)>,
    installs: HashMap<String, i32>,
}

impl Stored {
    /// A member of `team` sees it, and so does `user` once it is unlisted
    /// when they installed it.
    fn seen_by(&self, user: &MacroUserIdStr<'_>, team: Option<Uuid>) -> bool {
        Some(self.listing.team_id) == team
            && (!self.removed || self.installs.contains_key(user.as_ref()))
    }

    fn as_seen_by(&self, user: &MacroUserIdStr<'_>) -> Listing {
        Listing {
            id: self.listing.id,
            kind: self.listing.kind,
            scope: ListingScope::Team {
                team_id: self.listing.team_id,
            },
            publisher: self.listing.publisher.clone(),
            source_document_id: self.listing.source_document_id,
            title: self.listing.text.title.clone(),
            description: self.listing.text.description.clone(),
            latest_version: self.newest().0,
            installed_major: self.installs.get(user.as_ref()).copied(),
            install_count: self.installs.len() as i64,
            removed: self.removed,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn newest(&self) -> &(Version, PublishedPage) {
        self.versions
            .iter()
            .max_by_key(|(version, _)| *version)
            .expect("a listing has a version")
    }
}

#[derive(Default)]
struct FakeListings(Mutex<Vec<Stored>>);

#[derive(Debug, thiserror::Error)]
#[error("fake repository failed")]
struct FakeErr;

impl ListingRepo for FakeListings {
    type Err = FakeErr;

    async fn create_listing(&self, listing: NewListing) -> Result<bool, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let listed = stored.iter().any(|existing| {
            !existing.removed && existing.listing.source_document_id == listing.source_document_id
        });
        if listed {
            return Ok(false);
        }

        let first = (Version::FIRST, listing.page.clone());
        stored.push(Stored {
            listing,
            removed: false,
            versions: vec![first],
            installs: HashMap::new(),
        });
        Ok(true)
    }

    async fn listing(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        id: ListingId,
    ) -> Result<Option<Listing>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .find(|entry| entry.listing.id == id && entry.seen_by(user, team))
            .map(|entry| entry.as_seen_by(user)))
    }

    async fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        query: &ListingQuery,
    ) -> Result<ListingPage, Self::Err> {
        let stored = self.0.lock().unwrap();
        let listings = stored
            .iter()
            .filter(|entry| entry.seen_by(user, team))
            .filter(|entry| match query.shown {
                ListingsShown::Listed => !entry.removed,
                ListingsShown::Installed => entry.installs.contains_key(user.as_ref()),
            })
            .map(|entry| entry.as_seen_by(user))
            .collect();
        Ok(ListingPage {
            listings,
            next: None,
        })
    }

    async fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        page: &PublishedPage,
    ) -> Result<AddedVersion, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let Some(entry) = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id && !entry.removed)
        else {
            return Ok(AddedVersion::Missing);
        };

        let (newest, newest_page) = entry.newest();
        if newest_page.source_hash == page.source_hash {
            return Ok(AddedVersion::Unchanged);
        }

        let next = newest.next(bump);
        entry.versions.push((next, page.clone()));
        Ok(AddedVersion::Added(next))
    }

    async fn remove_listing(&self, id: ListingId) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            entry.removed = true;
        }
        Ok(())
    }

    async fn install_latest(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            let major = entry.newest().0.major;
            entry.installs.insert(user.as_ref().to_owned(), major);
        }
        Ok(())
    }

    async fn uninstall(&self, id: ListingId, user: &MacroUserIdStr<'_>) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            entry.installs.remove(user.as_ref());
        }
        Ok(())
    }

    async fn installed_page(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
    ) -> Result<Option<InstalledPage>, Self::Err> {
        let stored = self.0.lock().unwrap();
        let Some(entry) = stored
            .iter()
            .find(|entry| entry.listing.id == id && Some(entry.listing.team_id) == team)
        else {
            return Ok(None);
        };
        let Some(major) = entry.installs.get(user.as_ref()) else {
            return Ok(None);
        };

        Ok(entry
            .versions
            .iter()
            .filter(|(version, _)| version.major == *major)
            .max_by_key(|(version, _)| *version)
            .map(|(version, page)| InstalledPage {
                version: *version,
                html: page.html.clone(),
            }))
    }
}

/// Documents by id; a source containing `broken` doesn't build.
#[derive(Default)]
struct FakePages(Mutex<HashMap<Uuid, String>>);

impl FakePages {
    fn save(&self, document: Uuid, html: &str) {
        self.0.lock().unwrap().insert(document, html.to_owned());
    }
}

impl Pages for FakePages {
    async fn source(
        &self,
        document: &EntityAccessReceipt<ViewAccessLevel>,
    ) -> Result<String, DocumentProgramError> {
        let id = Uuid::parse_str(&document.entity().entity_id).expect("a document id");
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(DocumentProgramError::UnknownProgram)
    }

    async fn build(&self, html: &str) -> Result<DocumentBuild, DocumentProgramError> {
        let outcome = if html.contains("broken") {
            BuildOutcome::Failed(Vec::new())
        } else {
            BuildOutcome::Plain
        };
        Ok(DocumentBuild {
            source_hash: SourceHash::of(html),
            outcome: Arc::new(outcome),
        })
    }

    async fn run(
        &self,
        _user: &MacroUserIdStr<'static>,
        _html: &str,
        _request: RunProgram,
        _cancel: CancellationToken,
    ) -> Result<UserExecutionReceipt, DocumentProgramError> {
        Err(DocumentProgramError::UnknownProgram)
    }
}

struct FakeRollout(bool);

impl Rollout for FakeRollout {
    async fn allows(&self, _user: &MacroUserIdStr<'static>) -> bool {
        self.0
    }
}

type Service = MarketplaceServiceImpl<FakeListings, FakePages, FakeRollout>;

fn marketplace() -> (Service, Arc<FakePages>) {
    let pages = Arc::new(FakePages::default());
    let service = MarketplaceServiceImpl::new(
        FakeListings::default(),
        Arc::clone(&pages),
        FakeRollout(true),
    );
    (service, pages)
}

/// Ada, a member of Acme, publishes her board.
async fn published_board(service: &Service, pages: &FakePages) -> Listing {
    pages.save(BOARD, "<h1>Board</h1>");
    service
        .publish(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            owned(ADA, BOARD),
            text("Board"),
        )
        .await
        .expect("the board publishes")
}

#[tokio::test]
async fn publishing_needs_a_team() {
    let (service, pages) = marketplace();
    pages.save(BOARD, "<h1>Board</h1>");

    let refused = service
        .publish(&user(ADA), None, owned(ADA, BOARD), text("Board"))
        .await;

    assert!(matches!(refused, Err(MarketplaceError::NoTeam)));
}

#[tokio::test]
async fn a_first_version_is_one_point_zero_and_a_document_lists_once() {
    let (service, pages) = marketplace();

    let listing = published_board(&service, &pages).await;
    let again = service
        .publish(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            owned(ADA, BOARD),
            text("Board again"),
        )
        .await;

    assert_eq!(listing.latest_version, Version { major: 1, minor: 0 });
    assert_eq!(listing.installed_major, None);
    assert!(matches!(again, Err(MarketplaceError::AlreadyPublished)));
}

#[tokio::test]
async fn a_page_that_does_not_build_is_not_published() {
    let (service, pages) = marketplace();
    pages.save(BOARD, "<script>broken</script>");

    let refused = service
        .publish(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            owned(ADA, BOARD),
            text("Board"),
        )
        .await;

    assert!(matches!(refused, Err(MarketplaceError::DoesNotBuild)));
}

#[tokio::test]
async fn only_the_team_sees_and_installs_a_listing() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ben = member(BEN, ACME, TeamRole::Member);
    let cam = member(CAM, OTHER_TEAM, TeamRole::Member);

    let installed = service
        .install(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("a teammate installs it");
    let outside = service.install(&user(CAM), Some(&cam), listing.id).await;
    let outside_listings = service
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .listings;

    assert_eq!(installed.installed_major, Some(1));
    assert_eq!(installed.install_count, 1);
    assert!(matches!(outside, Err(MarketplaceError::NotFound)));
    assert!(outside_listings.is_empty());
}

#[tokio::test]
async fn a_minor_version_reaches_an_install_at_once_and_a_major_waits() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ada = member(ADA, ACME, TeamRole::Member);
    let ben = member(BEN, ACME, TeamRole::Member);
    service
        .install(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("a teammate installs it");

    pages.save(BOARD, "<h1>Board, tidied</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
        )
        .await
        .expect("a minor version publishes");
    let after_minor = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");

    pages.save(BOARD, "<h1>Board, rebuilt</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Major,
        )
        .await
        .expect("a major version publishes");
    let after_major = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");

    let taken = service
        .install(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the major version is taken");
    let after_taking = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");

    assert_eq!(after_minor.version, Version { major: 1, minor: 1 });
    assert_eq!(after_minor.html, "<h1>Board, tidied</h1>");
    assert_eq!(after_major.version, Version { major: 1, minor: 1 });
    assert_eq!(after_major.html, "<h1>Board, tidied</h1>");
    assert_eq!(taken.installed_major, Some(2));
    assert_eq!(after_taking.version, Version { major: 2, minor: 0 });
    assert_eq!(after_taking.html, "<h1>Board, rebuilt</h1>");
}

#[tokio::test]
async fn a_version_needs_the_publisher_a_change_and_the_same_document() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ada = member(ADA, ACME, TeamRole::Member);
    let other_document = Uuid::from_u128(11);
    pages.save(other_document, "<h1>Something else</h1>");

    let unchanged = service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
        )
        .await;
    let by_teammate = service
        .publish_version(
            &user(BEN),
            Some(&member(BEN, ACME, TeamRole::Admin)),
            listing.id,
            owned(BEN, BOARD),
            VersionBump::Minor,
        )
        .await;
    let from_another_document = service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, other_document),
            VersionBump::Minor,
        )
        .await;

    assert!(matches!(unchanged, Err(MarketplaceError::Unchanged)));
    assert!(matches!(by_teammate, Err(MarketplaceError::Forbidden)));
    assert!(matches!(
        from_another_document,
        Err(MarketplaceError::Invalid(_))
    ));
}

#[tokio::test]
async fn the_publisher_or_a_team_admin_unlists_and_installs_keep_working() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ben = member(BEN, ACME, TeamRole::Member);
    let cam = member(CAM, ACME, TeamRole::Member);
    service
        .install(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("a teammate installs it");

    let by_member = service.unlist(&user(CAM), Some(&cam), listing.id).await;
    service
        .unlist(
            &user(CAM),
            Some(&member(CAM, ACME, TeamRole::Admin)),
            listing.id,
        )
        .await
        .expect("a team admin unlists it");

    let for_newcomer = service
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .listings;
    let for_installer = service
        .listings(&user(BEN), Some(&ben), first_page(ListingsShown::Installed))
        .await
        .expect("listings load")
        .listings;
    let still_builds = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await;

    assert!(matches!(by_member, Err(MarketplaceError::Forbidden)));
    assert!(for_newcomer.is_empty());
    assert_eq!(for_installer.len(), 1);
    assert!(for_installer[0].removed);
    assert!(still_builds.is_ok());
}

#[tokio::test]
async fn a_page_runs_only_for_someone_who_installed_it() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ben = member(BEN, ACME, TeamRole::Member);
    let request = || RunProgram {
        source_hash: SourceHash::of("<h1>Board</h1>"),
        program: ProgramId::try_new("tasks".to_owned()).expect("a program id"),
        arguments: serde_json::Map::new(),
        timeout_ms: 1_000,
    };

    let before = service
        .run_installed(
            &user(BEN),
            Some(&ben),
            listing.id,
            request(),
            CancellationToken::new(),
        )
        .await;
    service
        .install(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("a teammate installs it");
    let after = service
        .run_installed(
            &user(BEN),
            Some(&ben),
            listing.id,
            request(),
            CancellationToken::new(),
        )
        .await;

    assert!(matches!(before, Err(MarketplaceError::NotFound)));
    // The fake page has no programs: reaching it is the point.
    assert!(matches!(
        after,
        Err(MarketplaceError::Page(DocumentProgramError::UnknownProgram))
    ));
}

#[tokio::test]
async fn nobody_outside_the_rollout_sees_the_marketplace() {
    let service = MarketplaceServiceImpl::new(
        FakeListings::default(),
        Arc::new(FakePages::default()),
        FakeRollout(false),
    );

    let listings = service
        .listings(
            &user(ADA),
            Some(&member(ADA, ACME, TeamRole::Member)),
            first_page(ListingsShown::Listed),
        )
        .await;

    assert!(matches!(listings, Err(MarketplaceError::NotFound)));
}
