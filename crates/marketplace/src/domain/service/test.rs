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
use scheduled_action::domain::templates::{RoutineSnapshot, TemplateVariable, fill::FilledRoutine};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::MarketplaceServiceImpl;
use crate::domain::models::{
    AddedVersion, FollowingInstall, InstallRequest, InstallUpdate, InstalledPage, InstalledRoutine,
    Listing, ListingContent, ListingId, ListingKind, ListingScope, ListingText, MarketplaceError,
    NewListing, PublishSource, PublishedVersion, RoutineCopy, RoutineInstall, RoutineUpdate,
    RoutineVersion, TaskHash, Version, VersionBump,
};
use crate::domain::ports::{ListingRepo, MarketplaceService, Pages, Rollout, Routines};

mod routines;

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

fn owned(id: &'static str, document: Uuid) -> PublishSource {
    PublishSource::Artifact(EntityAccessReceipt::dangerously_assert_authenticated_user(
        user(id),
        &document.to_string(),
        EntityType::Document,
    ))
}

fn text(title: &str) -> ListingText {
    ListingText {
        title: title.to_owned(),
        description: String::new(),
    }
}

/// A person's install: the major they are on, and the routine it made.
#[derive(Clone)]
struct StoredInstall {
    major: i32,
    routine: Option<RoutineInstall>,
    detached: bool,
    update_failed: bool,
}

struct Stored {
    listing: NewListing,
    removed: bool,
    versions: Vec<(Version, ListingContent)>,
    installs: HashMap<String, StoredInstall>,
}

impl Stored {
    fn newest(&self) -> &(Version, ListingContent) {
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
            !existing.removed
                && existing.listing.source_id == listing.source_id
                && existing.listing.content.kind() == listing.content.kind()
        });
        if listed {
            return Ok(false);
        }

        let first = (Version::FIRST, listing.content.clone());
        stored.push(Stored {
            listing,
            removed: false,
            versions: vec![first],
            installs: HashMap::new(),
        });
        Ok(true)
    }

    async fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        team: Option<Uuid>,
        only: Option<ListingId>,
    ) -> Result<Vec<Listing>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .filter(|entry| Some(entry.listing.team_id) == team)
            .filter(|entry| !entry.removed || entry.installs.contains_key(user.as_ref()))
            .filter(|entry| only.is_none_or(|id| id == entry.listing.id))
            .map(|entry| Listing {
                id: entry.listing.id,
                kind: entry.listing.content.kind(),
                scope: ListingScope::Team {
                    team_id: entry.listing.team_id,
                },
                publisher: entry.listing.publisher.clone(),
                source_id: entry.listing.source_id,
                title: entry.listing.text.title.clone(),
                description: entry.listing.text.description.clone(),
                latest_version: entry.newest().0,
                installed_major: entry
                    .installs
                    .get(user.as_ref())
                    .map(|install| install.major),
                installed_routine: entry.installs.get(user.as_ref()).and_then(|install| {
                    install.routine.as_ref().map(|routine| InstalledRoutine {
                        routine_id: routine.routine_id,
                        detached: install.detached,
                        update_failed: install.update_failed,
                    })
                }),
                install_count: entry.installs.len() as i64,
                removed: entry.removed,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .collect())
    }

    async fn add_version(
        &self,
        id: ListingId,
        bump: VersionBump,
        content: &ListingContent,
    ) -> Result<AddedVersion, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let Some(entry) = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id && !entry.removed)
        else {
            return Ok(AddedVersion::Missing);
        };

        let (newest, newest_content) = entry.newest();
        if newest_content.source_hash() == content.source_hash() {
            return Ok(AddedVersion::Unchanged);
        }

        let next = newest.next(bump);
        entry.versions.push((next, content.clone()));
        Ok(AddedVersion::Added(next))
    }

    async fn routine_version(
        &self,
        id: ListingId,
        major: Option<i32>,
    ) -> Result<Option<RoutineVersion>, Self::Err> {
        let stored = self.0.lock().unwrap();
        let Some(entry) = stored.iter().find(|entry| entry.listing.id == id) else {
            return Ok(None);
        };

        Ok(entry
            .versions
            .iter()
            .filter(|(version, _)| major.is_none_or(|major| version.major == major))
            .max_by_key(|(version, _)| *version)
            .and_then(|(version, content)| match content {
                ListingContent::Routine(routine) => Some(RoutineVersion {
                    version: *version,
                    snapshot: routine.snapshot.clone(),
                }),
                ListingContent::Page(_) => None,
            }))
    }

    async fn routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<Option<RoutineInstall>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .find(|entry| entry.listing.id == id)
            .and_then(|entry| entry.installs.get(user.as_ref()))
            .and_then(|install| install.routine.clone()))
    }

    async fn save_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        install: &RoutineInstall,
    ) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            entry.installs.insert(
                user.as_ref().to_owned(),
                StoredInstall {
                    major: install.version.major,
                    routine: Some(install.clone()),
                    detached: false,
                    update_failed: false,
                },
            );
        }
        Ok(())
    }

    async fn installs_behind(
        &self,
        id: ListingId,
        version: Version,
    ) -> Result<Vec<FollowingInstall>, Self::Err> {
        let stored = self.0.lock().unwrap();
        let Some(entry) = stored.iter().find(|entry| entry.listing.id == id) else {
            return Ok(Vec::new());
        };

        Ok(entry
            .installs
            .iter()
            .filter(|(_, install)| !install.detached)
            .filter_map(|(person, install)| {
                let routine = install.routine.clone()?;
                let behind =
                    routine.version.major == version.major && routine.version.minor < version.minor;
                behind.then(|| FollowingInstall {
                    user: MacroUserIdStr::try_from(person.clone()).expect("a stored user id"),
                    install: routine,
                })
            })
            .collect())
    }

    async fn record_update(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        update: &InstallUpdate,
    ) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let install = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id)
            .and_then(|entry| entry.installs.get_mut(user.as_ref()))
            .expect("an install to record");
        match update {
            InstallUpdate::Updated { version, task_hash } => {
                if let Some(routine) = &mut install.routine {
                    routine.version = *version;
                    routine.task_hash = task_hash.clone();
                }
                install.update_failed = false;
            }
            InstallUpdate::Detached => {
                install.detached = true;
                install.update_failed = false;
            }
            InstallUpdate::Failed => install.update_failed = true,
        }
        Ok(())
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
            entry.installs.insert(
                user.as_ref().to_owned(),
                StoredInstall {
                    major,
                    routine: None,
                    detached: false,
                    update_failed: false,
                },
            );
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
        let Some(install) = entry.installs.get(user.as_ref()) else {
            return Ok(None);
        };

        Ok(entry
            .versions
            .iter()
            .filter(|(version, _)| version.major == install.major)
            .max_by_key(|(version, _)| *version)
            .and_then(|(version, content)| match content {
                ListingContent::Page(page) => Some(InstalledPage {
                    version: *version,
                    html: page.html.clone(),
                }),
                ListingContent::Routine(_) => None,
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

/// Routines by id, each with its owner, filled-in routine and task hash.
#[derive(Default)]
struct FakeRoutines {
    /// Routines that can be published: their owner and snapshot.
    sources: Mutex<HashMap<Uuid, (String, RoutineSnapshot)>>,
    /// Routines installs made.
    saved: Mutex<HashMap<Uuid, SavedRoutine>>,
    /// Saving one of these fails.
    failing: Mutex<Vec<Uuid>>,
}

#[derive(Clone, Debug)]
struct SavedRoutine {
    owner: String,
    routine: FilledRoutine,
    enabled: bool,
    revision: u32,
}

impl SavedRoutine {
    fn hash(&self) -> TaskHash {
        TaskHash(self.routine.task.to_string())
    }
}

impl FakeRoutines {
    fn save_source(&self, owner: &str, id: Uuid, snapshot: RoutineSnapshot) {
        self.sources
            .lock()
            .unwrap()
            .insert(id, (owner.to_owned(), snapshot));
    }

    fn saved(&self, id: Uuid) -> SavedRoutine {
        self.saved.lock().unwrap()[&id].clone()
    }

    /// The person edits their routine's task.
    fn edit(&self, id: Uuid, task: serde_json::Value) {
        self.saved
            .lock()
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .routine
            .task = task;
    }

    fn fail(&self, id: Uuid) {
        self.failing.lock().unwrap().push(id);
    }
}

impl Routines for FakeRoutines {
    async fn snapshot(
        &self,
        routine: &EntityAccessReceipt<OwnerAccessLevel>,
        description: &str,
        variables: Vec<TemplateVariable>,
    ) -> Result<RoutineSnapshot, MarketplaceError> {
        let id = Uuid::parse_str(&routine.entity().entity_id).expect("a routine id");
        let (owner, snapshot) = self
            .sources
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(MarketplaceError::NotFound)?;
        if routine
            .get_authenticated_user()
            .ok()
            .map(|user| user.as_ref())
            != Some(owner.as_str())
        {
            return Err(MarketplaceError::NotFound);
        }

        Ok(RoutineSnapshot {
            description: description.to_owned(),
            variables,
            ..snapshot
        })
    }

    async fn create(
        &self,
        user: &MacroUserIdStr<'static>,
        routine: &FilledRoutine,
        enabled: bool,
    ) -> Result<RoutineCopy, MarketplaceError> {
        let saved = SavedRoutine {
            owner: user.as_ref().to_owned(),
            routine: routine.clone(),
            enabled,
            revision: 1,
        };
        let routine_id = Uuid::now_v7();
        let task_hash = saved.hash();
        self.saved.lock().unwrap().insert(routine_id, saved);
        Ok(RoutineCopy {
            routine_id,
            task_hash,
        })
    }

    async fn update(
        &self,
        user: &MacroUserIdStr<'static>,
        routine_id: Uuid,
        routine: &FilledRoutine,
        unedited: Option<&TaskHash>,
    ) -> Result<RoutineUpdate, MarketplaceError> {
        if self.failing.lock().unwrap().contains(&routine_id) {
            return Err(MarketplaceError::RoutineRejected(
                "connect Linear first".to_owned(),
            ));
        }
        let mut saved = self.saved.lock().unwrap();
        let Some(current) = saved
            .get_mut(&routine_id)
            .filter(|current| current.owner == user.as_ref())
        else {
            return Ok(RoutineUpdate::Gone);
        };
        if unedited.is_some_and(|expected| current.hash() != *expected) {
            return Ok(RoutineUpdate::Edited);
        }

        current.routine = routine.clone();
        current.revision += 1;
        Ok(RoutineUpdate::Updated(current.hash()))
    }
}

type Service = MarketplaceServiceImpl<FakeListings, FakePages, FakeRoutines, FakeRollout>;

fn marketplace() -> (Service, Arc<FakePages>) {
    let (service, pages, _) = marketplace_with_routines();
    (service, pages)
}

fn marketplace_with_routines() -> (Service, Arc<FakePages>, Arc<FakeRoutines>) {
    let pages = Arc::new(FakePages::default());
    let routines = Arc::new(FakeRoutines::default());
    let service = MarketplaceServiceImpl::new(
        FakeListings::default(),
        Arc::clone(&pages),
        Arc::clone(&routines),
        FakeRollout(true),
    );
    (service, pages, routines)
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
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("a teammate installs it");
    let outside = service
        .install(
            &user(CAM),
            Some(&cam),
            listing.id,
            InstallRequest::default(),
        )
        .await;
    let outside_listings = service
        .listings(&user(CAM), Some(&cam))
        .await
        .expect("listings load");

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
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
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
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
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
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
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
        .listings(&user(CAM), Some(&cam))
        .await
        .expect("listings load");
    let for_installer = service
        .listings(&user(BEN), Some(&ben))
        .await
        .expect("listings load");
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
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
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
        Arc::new(FakeRoutines::default()),
        FakeRollout(false),
    );

    let listings = service
        .listings(&user(ADA), Some(&member(ADA, ACME, TeamRole::Member)))
        .await;

    assert!(matches!(listings, Err(MarketplaceError::NotFound)));
}
