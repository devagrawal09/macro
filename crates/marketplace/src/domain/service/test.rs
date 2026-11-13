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
    AddedVersion, Audience, Changelog, FollowingInstall, InstallRequest, InstallUpdate,
    InstalledPage, InstalledRoutine, Listing, ListingContent, ListingId, ListingKind,
    ListingOrigin, ListingPage, ListingQuery, ListingScope, ListingText, ListingVersion,
    ListingsShown, MarketplaceError, NewCopy, NewListing, PendingReview, PendingVersion,
    PublishScope, PublishSource, PublishedVersion, ReviewDecision, ReviewState, RoutineCopy,
    RoutineInstall, RoutineUpdate, RoutineVersion, TaskHash, Version, VersionBump, VersionNotes,
    VersionNotice,
};
use crate::domain::ports::{
    Documents, ListingRepo, MarketplaceService, Pages, Rollout, Routines, VersionNotifier,
};

mod review;
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

/// The first page of `shown`, as the web app asks for it.
fn first_page(shown: ListingsShown) -> ListingQuery {
    ListingQuery::new(shown, None, None, None, None, None).expect("a valid query")
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
    /// The minor it is pinned to.
    pinned: Option<i32>,
}

/// One stored version: its number, content, notes and review.
#[derive(Clone)]
struct StoredVersion {
    version: Version,
    content: ListingContent,
    notes: VersionNotes,
    review: ReviewState,
    reviewer: Option<String>,
    reason: Option<String>,
}

impl StoredVersion {
    fn new(
        version: Version,
        content: ListingContent,
        notes: VersionNotes,
        review: ReviewState,
    ) -> Self {
        Self {
            version,
            content,
            notes,
            review,
            reviewer: None,
            reason: None,
        }
    }

    /// The version as a history lists it.
    fn listed(&self) -> ListingVersion {
        ListingVersion {
            version: self.version,
            changelog: self.notes.changelog.as_deref().map(str::to_owned),
            forced_major: self.notes.forced_major,
            review: self.review,
            created_at: Utc::now(),
        }
    }
}

struct Stored {
    listing: NewListing,
    removed: bool,
    versions: Vec<StoredVersion>,
    installs: HashMap<String, StoredInstall>,
    /// People whose first routine install is creating its routine.
    claims: std::collections::HashSet<String>,
}

impl Stored {
    /// A member of `team` sees its team's and global listings once approved,
    /// a global publisher also global ones waiting, and `user` an unlisted
    /// one they installed.
    fn seen_by(&self, user: &MacroUserIdStr<'_>, audience: Audience) -> bool {
        let global = self.listing.team_id.is_none();
        let in_scope = global || self.listing.team_id == audience.team;
        let reviewed = self.approved().is_some() || (global && audience.global_publisher);
        in_scope && reviewed && (!self.removed || self.installs.contains_key(user.as_ref()))
    }

    fn as_seen_by(&self, user: &MacroUserIdStr<'_>, audience: Audience) -> Listing {
        let install = self.installs.get(user.as_ref());
        let pending = audience
            .global_publisher
            .then(|| self.pending().map(|pending| pending.version))
            .flatten();
        Listing {
            id: self.listing.id,
            kind: self.listing.content.kind(),
            scope: match self.listing.team_id {
                Some(team_id) => ListingScope::Team { team_id },
                None => ListingScope::Global,
            },
            publisher: self.listing.publisher.clone(),
            source_id: self.listing.source_id,
            title: self.listing.text.title.clone(),
            description: self.listing.text.description.clone(),
            latest_version: self
                .approved()
                .map(|approved| approved.version)
                .or(pending)
                .expect("a seen listing has a version to show"),
            pending_version: pending,
            installed_major: install.map(|install| install.major),
            installed_version: install.and_then(|install| self.installed_version(install)),
            pinned: install.is_some_and(|install| install.pinned.is_some()),
            based_on: None,
            installed_routine: install.and_then(|install| {
                install.routine.as_ref().map(|routine| InstalledRoutine {
                    routine_id: routine.routine_id,
                    detached: install.detached,
                    update_failed: install.update_failed,
                })
            }),
            install_count: self.installs.len() as i64,
            removed: self.removed,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// The version an install has: its routine's, the one it is pinned to,
    /// or its major's newest approved one.
    fn installed_version(&self, install: &StoredInstall) -> Option<Version> {
        let pinned = install.pinned.map(|minor| Version {
            major: install.major,
            minor,
        });
        install
            .routine
            .as_ref()
            .map(|routine| routine.version)
            .or(pinned)
            .or_else(|| {
                self.versions
                    .iter()
                    .filter(|stored| stored.review == ReviewState::Approved)
                    .map(|stored| stored.version)
                    .filter(|version| version.major == install.major)
                    .max()
            })
    }

    /// The newest version not rejected, which the next is numbered after.
    fn newest(&self) -> Option<&StoredVersion> {
        self.versions
            .iter()
            .filter(|stored| stored.review != ReviewState::Rejected)
            .max_by_key(|stored| stored.version)
    }

    fn approved(&self) -> Option<&StoredVersion> {
        self.versions
            .iter()
            .filter(|stored| stored.review == ReviewState::Approved)
            .max_by_key(|stored| stored.version)
    }

    fn pending(&self) -> Option<&StoredVersion> {
        self.versions
            .iter()
            .find(|stored| stored.review == ReviewState::Pending)
    }

    fn pending_version(&self) -> Option<PendingVersion> {
        let pending = self.pending()?;
        Some(PendingVersion {
            listing_id: self.listing.id,
            kind: self.listing.content.kind(),
            title: self.listing.text.title.clone(),
            description: self.listing.text.description.clone(),
            publisher: self.listing.publisher.clone(),
            version: pending.version,
            approved_version: self.approved().map(|approved| approved.version),
            published_at: Utc::now(),
        })
    }
}

/// `entry` as `user` sees it: with the listing it was copied from when they
/// may see that one too, by the same rules.
fn seen(entry: &Stored, all: &[Stored], user: &MacroUserIdStr<'_>, audience: Audience) -> Listing {
    let based_on = entry.listing.derived_from.and_then(|origin| {
        all.iter()
            .find(|other| other.listing.id == origin && other.seen_by(user, audience))
            .map(|other| ListingOrigin {
                id: origin,
                title: other.listing.text.title.clone(),
            })
    });
    Listing {
        based_on,
        ..entry.as_seen_by(user, audience)
    }
}

/// Listings, and the copies people made of them.
#[derive(Default)]
struct FakeListings(Mutex<Vec<Stored>>, Mutex<Vec<NewCopy>>);

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

        let first = StoredVersion::new(
            Version::FIRST,
            listing.content.clone(),
            VersionNotes {
                changelog: listing.changelog.clone(),
                forced_major: false,
            },
            listing.review,
        );
        stored.push(Stored {
            listing,
            removed: false,
            versions: vec![first],
            installs: HashMap::new(),
            claims: std::collections::HashSet::new(),
        });
        Ok(true)
    }

    async fn listing(
        &self,
        user: &MacroUserIdStr<'_>,
        audience: Audience,
        id: ListingId,
    ) -> Result<Option<Listing>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .find(|entry| entry.listing.id == id && entry.seen_by(user, audience))
            .map(|entry| seen(entry, &stored, user, audience)))
    }

    async fn listings(
        &self,
        user: &MacroUserIdStr<'_>,
        audience: Audience,
        query: &ListingQuery,
    ) -> Result<ListingPage, Self::Err> {
        let stored = self.0.lock().unwrap();
        let listings = stored
            .iter()
            .filter(|entry| entry.seen_by(user, audience))
            .filter(|entry| match query.shown {
                ListingsShown::Listed => !entry.removed,
                ListingsShown::Installed => entry.installs.contains_key(user.as_ref()),
            })
            .filter(|entry| {
                query
                    .kind
                    .is_none_or(|kind| kind == entry.listing.content.kind())
            })
            .filter(|entry| {
                query
                    .source_id
                    .is_none_or(|id| id == entry.listing.source_id)
            })
            .map(|entry| seen(entry, &stored, user, audience))
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
        content: &ListingContent,
        review: ReviewState,
        notes: &VersionNotes,
    ) -> Result<AddedVersion, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let Some(entry) = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id && !entry.removed)
        else {
            return Ok(AddedVersion::Missing);
        };
        if entry.pending().is_some() {
            return Ok(AddedVersion::ReviewPending);
        }

        let Some(newest) = entry.newest() else {
            return Ok(AddedVersion::Missing);
        };
        if newest.content.source_hash() == content.source_hash() {
            return Ok(AddedVersion::Unchanged);
        }

        let next = newest.version.next(bump);
        entry.versions.push(StoredVersion::new(
            next,
            content.clone(),
            notes.clone(),
            review,
        ));
        Ok(AddedVersion::Added(next))
    }

    async fn pending_versions(&self, limit: u16) -> Result<Vec<PendingVersion>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .filter(|entry| !entry.removed)
            .filter_map(Stored::pending_version)
            .take(usize::from(limit))
            .collect())
    }

    async fn pending_review(&self, id: ListingId) -> Result<Option<PendingReview>, Self::Err> {
        let stored = self.0.lock().unwrap();
        Ok(stored
            .iter()
            .find(|entry| entry.listing.id == id && !entry.removed)
            .and_then(|entry| {
                Some(PendingReview {
                    pending: entry.pending_version()?,
                    content: entry.pending()?.content.clone(),
                })
            }))
    }

    async fn decide_review(
        &self,
        id: ListingId,
        version: Version,
        reviewer: &MacroUserIdStr<'_>,
        decision: &ReviewDecision,
    ) -> Result<bool, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let Some(pending) = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id)
            .and_then(|entry| {
                entry.versions.iter_mut().find(|stored| {
                    stored.version == version && stored.review == ReviewState::Pending
                })
            })
        else {
            return Ok(false);
        };

        pending.review = decision.state();
        pending.reviewer = Some(reviewer.as_ref().to_owned());
        pending.reason = match decision {
            ReviewDecision::Approve => None,
            ReviewDecision::Reject { reason } => reason.clone(),
        };
        Ok(true)
    }

    async fn versions(
        &self,
        id: ListingId,
        include_pending: bool,
    ) -> Result<Vec<ListingVersion>, Self::Err> {
        let stored = self.0.lock().unwrap();
        let Some(entry) = stored.iter().find(|entry| entry.listing.id == id) else {
            return Ok(Vec::new());
        };
        let mut versions: Vec<ListingVersion> = entry
            .versions
            .iter()
            .filter(|stored| match stored.review {
                ReviewState::Approved => true,
                ReviewState::Pending => include_pending,
                ReviewState::Rejected => false,
            })
            .map(StoredVersion::listed)
            .collect();
        versions.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(versions)
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
            .filter(|stored| stored.review == ReviewState::Approved)
            .filter(|stored| major.is_none_or(|major| stored.version.major == major))
            .max_by_key(|stored| stored.version)
            .and_then(|stored| match &stored.content {
                ListingContent::Routine(routine) => Some(RoutineVersion {
                    version: stored.version,
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

    async fn claim_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        _major: i32,
        _stale_after_secs: u32,
    ) -> Result<bool, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let entry = stored
            .iter_mut()
            .find(|entry| entry.listing.id == id)
            .expect("a listing to claim");
        if entry.installs.contains_key(user.as_ref()) {
            return Ok(false);
        }
        Ok(entry.claims.insert(user.as_ref().to_owned()))
    }

    async fn release_routine_claim(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
    ) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            entry.claims.remove(user.as_ref());
        }
        Ok(())
    }

    async fn save_routine_install(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        install: &RoutineInstall,
    ) -> Result<(), Self::Err> {
        let mut stored = self.0.lock().unwrap();
        if let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) {
            entry.claims.remove(user.as_ref());
            entry.installs.insert(
                user.as_ref().to_owned(),
                StoredInstall {
                    major: install.version.major,
                    routine: Some(install.clone()),
                    detached: false,
                    update_failed: false,
                    pinned: None,
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
            .filter(|(_, install)| !install.detached && install.pinned.is_none())
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

    async fn set_pinned(
        &self,
        id: ListingId,
        user: &MacroUserIdStr<'_>,
        pinned: bool,
    ) -> Result<bool, Self::Err> {
        let mut stored = self.0.lock().unwrap();
        let Some(entry) = stored.iter_mut().find(|entry| entry.listing.id == id) else {
            return Ok(false);
        };
        let Some(install) = entry.installs.get(user.as_ref()).cloned() else {
            return Ok(false);
        };
        let has = entry
            .installed_version(&install)
            .expect("an install has an approved version");
        let pin = pinned.then(|| install.pinned.unwrap_or(has.minor));
        if let Some(install) = entry.installs.get_mut(user.as_ref()) {
            install.pinned = pin;
        }
        Ok(true)
    }

    async fn record_copy(&self, copy: &NewCopy) -> Result<(), Self::Err> {
        let mut copies = self.1.lock().unwrap();
        let recorded = copies
            .iter()
            .any(|existing| existing.kind == copy.kind && existing.copy_id == copy.copy_id);
        if !recorded {
            copies.push(copy.clone());
        }
        Ok(())
    }

    async fn copied_from(
        &self,
        kind: ListingKind,
        copy_id: Uuid,
    ) -> Result<Option<ListingId>, Self::Err> {
        Ok(self
            .1
            .lock()
            .unwrap()
            .iter()
            .find(|copy| copy.kind == kind && copy.copy_id == copy_id)
            .map(|copy| copy.listing_id))
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
            let Some(major) = entry.approved().map(|approved| approved.version.major) else {
                return Ok(());
            };
            entry.installs.insert(
                user.as_ref().to_owned(),
                StoredInstall {
                    major,
                    routine: None,
                    detached: false,
                    update_failed: false,
                    pinned: None,
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
        let Some(entry) = stored.iter().find(|entry| {
            entry.listing.id == id
                && (entry.listing.team_id.is_none() || entry.listing.team_id == team)
        }) else {
            return Ok(None);
        };
        let Some(install) = entry.installs.get(user.as_ref()) else {
            return Ok(None);
        };

        Ok(entry
            .versions
            .iter()
            .filter(|stored| stored.review == ReviewState::Approved)
            .filter(|stored| stored.version.major == install.major)
            .filter(|stored| {
                install
                    .pinned
                    .is_none_or(|minor| stored.version.minor == minor)
            })
            .max_by_key(|stored| stored.version)
            .and_then(|stored| match &stored.content {
                ListingContent::Page(page) => Some(InstalledPage {
                    version: stored.version,
                    html: page.html.clone(),
                    pinned: install.pinned.is_some(),
                }),
                ListingContent::Routine(_) => None,
            }))
    }

    async fn installers(
        &self,
        id: ListingId,
        major: Option<i32>,
        limit: u16,
    ) -> Result<Vec<MacroUserIdStr<'static>>, Self::Err> {
        let stored = self.0.lock().unwrap();
        let Some(entry) = stored.iter().find(|entry| entry.listing.id == id) else {
            return Ok(Vec::new());
        };

        let mut installers: Vec<MacroUserIdStr<'static>> = entry
            .installs
            .iter()
            .filter(|(_, installed)| major.is_none_or(|major| major == installed.major))
            .map(|(user, _)| MacroUserIdStr::try_from(user.clone()).expect("a stored user id"))
            .collect();
        installers.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        installers.truncate(usize::from(limit));
        Ok(installers)
    }
}

/// Every notification sent, as the version and its installers' ids; or a
/// failure for each, when `failing`.
#[derive(Default)]
struct FakeNotifier {
    sent: Mutex<Vec<(VersionNotice, Vec<String>)>>,
    failing: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("fake notifier failed")]
struct FakeNotifyErr;

impl VersionNotifier for Arc<FakeNotifier> {
    type Err = FakeNotifyErr;

    async fn notify(
        &self,
        version: VersionNotice,
        installers: &[MacroUserIdStr<'static>],
    ) -> Result<(), Self::Err> {
        if self.failing {
            return Err(FakeNotifyErr);
        }

        let users = installers
            .iter()
            .map(|user| user.as_ref().to_owned())
            .collect();
        self.sent.lock().unwrap().push((version, users));
        Ok(())
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
    /// Creating a routine fails.
    failing_creates: Mutex<bool>,
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

    /// The person deletes their routine.
    fn delete(&self, id: Uuid) {
        self.saved.lock().unwrap().remove(&id);
    }

    fn fail_creates(&self, failing: bool) {
        *self.failing_creates.lock().unwrap() = failing;
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
        if *self.failing_creates.lock().unwrap() {
            return Err(MarketplaceError::RoutineRejected(
                "connect Linear first".to_owned(),
            ));
        }
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

/// HTML documents people created, by id: owner, name and source.
#[derive(Default)]
struct FakeDocuments(Mutex<HashMap<Uuid, (String, String, String)>>);

impl FakeDocuments {
    fn get(&self, id: Uuid) -> (String, String, String) {
        self.0.lock().unwrap()[&id].clone()
    }
}

impl Documents for FakeDocuments {
    async fn create_html(
        &self,
        user: &MacroUserIdStr<'static>,
        name: &str,
        html: &str,
    ) -> Result<Uuid, MarketplaceError> {
        let id = Uuid::now_v7();
        self.0.lock().unwrap().insert(
            id,
            (user.as_ref().to_owned(), name.to_owned(), html.to_owned()),
        );
        Ok(id)
    }
}

type Service = MarketplaceServiceImpl<
    FakeListings,
    FakePages,
    FakeRoutines,
    FakeDocuments,
    FakeRollout,
    Arc<FakeNotifier>,
>;

fn marketplace() -> (Service, Arc<FakePages>) {
    let (service, pages, _, _) = marketplace_with(FakeNotifier::default());
    (service, pages)
}

fn marketplace_with_routines() -> (Service, Arc<FakePages>, Arc<FakeRoutines>) {
    let (service, pages, routines, _) = marketplace_with(FakeNotifier::default());
    (service, pages, routines)
}

fn notifying_marketplace(notifier: FakeNotifier) -> (Service, Arc<FakePages>, Arc<FakeNotifier>) {
    let (service, pages, _, notifier) = marketplace_with(notifier);
    (service, pages, notifier)
}

fn marketplace_with(
    notifier: FakeNotifier,
) -> (
    Service,
    Arc<FakePages>,
    Arc<FakeRoutines>,
    Arc<FakeNotifier>,
) {
    let (service, pages, routines, _, notifier) = marketplace_with_documents(notifier);
    (service, pages, routines, notifier)
}

fn marketplace_with_documents(
    notifier: FakeNotifier,
) -> (
    Service,
    Arc<FakePages>,
    Arc<FakeRoutines>,
    Arc<FakeDocuments>,
    Arc<FakeNotifier>,
) {
    let pages = Arc::new(FakePages::default());
    let routines = Arc::new(FakeRoutines::default());
    let documents = Arc::new(FakeDocuments::default());
    let notifier = Arc::new(notifier);
    let service = MarketplaceServiceImpl::new(
        FakeListings::default(),
        Arc::clone(&pages),
        Arc::clone(&routines),
        Arc::clone(&documents),
        FakeRollout(true),
        Arc::clone(&notifier),
    );
    (service, pages, routines, documents, notifier)
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
            PublishScope::Team,
            Changelog::default(),
        )
        .await
        .expect("the board publishes")
}

#[tokio::test]
async fn publishing_needs_a_team() {
    let (service, pages) = marketplace();
    pages.save(BOARD, "<h1>Board</h1>");

    let refused = service
        .publish(
            &user(ADA),
            None,
            owned(ADA, BOARD),
            text("Board"),
            PublishScope::Team,
            Changelog::default(),
        )
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
            PublishScope::Team,
            Changelog::default(),
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
            PublishScope::Team,
            Changelog::default(),
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
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .page
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
            Changelog::default(),
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
            Changelog::default(),
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
            Changelog::default(),
        )
        .await;
    let by_teammate = service
        .publish_version(
            &user(BEN),
            Some(&member(BEN, ACME, TeamRole::Admin)),
            listing.id,
            owned(BEN, BOARD),
            VersionBump::Minor,
            Changelog::default(),
        )
        .await;
    let from_another_document = service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, other_document),
            VersionBump::Minor,
            Changelog::default(),
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
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .page
        .listings;
    let for_installer = service
        .listings(&user(BEN), Some(&ben), first_page(ListingsShown::Installed))
        .await
        .expect("listings load")
        .page
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
        Arc::new(FakeDocuments::default()),
        FakeRollout(false),
        Arc::new(FakeNotifier::default()),
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

#[tokio::test]
async fn a_minor_version_notifies_its_major_and_a_major_version_notifies_everyone() {
    let (service, pages, notifier) = notifying_marketplace(FakeNotifier::default());
    let listing = published_board(&service, &pages).await;
    let ada = member(ADA, ACME, TeamRole::Member);
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
        .expect("Ben installs 1.0");

    pages.save(BOARD, "<h1>Board, rebuilt</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Major,
            Changelog::default(),
        )
        .await
        .expect("a major version publishes");
    service
        .install(
            &user(CAM),
            Some(&cam),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("Cam installs 2.0");

    pages.save(BOARD, "<h1>Board, rebuilt and tidied</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
            Changelog::default(),
        )
        .await
        .expect("a minor version publishes");

    let sent = notifier.sent.lock().unwrap().clone();
    assert_eq!(
        sent,
        vec![
            // Ben, on 1, hears of 2.0 so he may take it.
            (
                VersionNotice {
                    listing_id: listing.id,
                    version: Version { major: 2, minor: 0 },
                },
                vec![BEN.to_owned()],
            ),
            // Only Cam is on 2, so only his page changes.
            (
                VersionNotice {
                    listing_id: listing.id,
                    version: Version { major: 2, minor: 1 },
                },
                vec![CAM.to_owned()],
            ),
        ]
    );
}

#[tokio::test]
async fn a_version_publishes_when_its_notification_fails() {
    let (service, pages, _) = notifying_marketplace(FakeNotifier {
        failing: true,
        ..FakeNotifier::default()
    });
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
    let published = service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
            Changelog::default(),
        )
        .await
        .expect("the version publishes anyway");
    let shown = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");

    assert_eq!(
        published.listing.latest_version,
        Version { major: 1, minor: 1 }
    );
    assert_eq!(shown.version, Version { major: 1, minor: 1 });
}

#[tokio::test]
async fn a_history_shows_each_versions_changelog_to_whoever_sees_the_listing() {
    let (service, pages) = marketplace();
    let ada = member(ADA, ACME, TeamRole::Member);
    pages.save(BOARD, "<h1>Board</h1>");
    let listing = service
        .publish(
            &user(ADA),
            Some(&ada),
            owned(ADA, BOARD),
            text("Board"),
            PublishScope::Team,
            Changelog::new(Some("  First cut  ")).expect("a changelog"),
        )
        .await
        .expect("the board publishes");
    pages.save(BOARD, "<h1>Board, tidied</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
            Changelog::new(Some("Tidier columns")).expect("a changelog"),
        )
        .await
        .expect("a minor version publishes");

    let for_teammate = service
        .versions(
            &user(BEN),
            Some(&member(BEN, ACME, TeamRole::Member)),
            listing.id,
        )
        .await
        .expect("a teammate reads the history");
    let for_outsider = service
        .versions(
            &user(CAM),
            Some(&member(CAM, OTHER_TEAM, TeamRole::Member)),
            listing.id,
        )
        .await;

    let changelogs: Vec<(Version, Option<&str>)> = for_teammate
        .iter()
        .map(|entry| (entry.version, entry.changelog.as_deref()))
        .collect();
    assert_eq!(
        changelogs,
        vec![
            (Version { major: 1, minor: 1 }, Some("Tidier columns")),
            (Version::FIRST, Some("First cut")),
        ]
    );
    assert!(matches!(for_outsider, Err(MarketplaceError::NotFound)));
}

#[tokio::test]
async fn a_pinned_page_keeps_its_minor_until_unpinned_or_updated() {
    let (service, pages) = marketplace();
    let listing = published_board(&service, &pages).await;
    let ada = member(ADA, ACME, TeamRole::Member);
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
    let not_installed = service
        .set_pinned(&user(CAM), Some(&cam), listing.id, true)
        .await;

    let pinned = service
        .set_pinned(&user(BEN), Some(&ben), listing.id, true)
        .await
        .expect("Ben pins his install");
    pages.save(BOARD, "<h1>Board, tidied</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
            Changelog::default(),
        )
        .await
        .expect("a minor version publishes");
    let while_pinned = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");
    let unpinned = service
        .set_pinned(&user(BEN), Some(&ben), listing.id, false)
        .await
        .expect("Ben unpins his install");
    let after_unpinning = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install builds");

    service
        .set_pinned(&user(BEN), Some(&ben), listing.id, true)
        .await
        .expect("Ben pins again");
    let updated = service
        .install(
            &user(BEN),
            Some(&ben),
            listing.id,
            InstallRequest::default(),
        )
        .await
        .expect("taking the newest version");

    assert!(matches!(not_installed, Err(MarketplaceError::NotFound)));
    assert!(pinned.pinned);
    assert_eq!(pinned.installed_version, Some(Version::FIRST));
    assert_eq!(while_pinned.version, Version::FIRST);
    assert!(while_pinned.pinned);
    assert_eq!(while_pinned.html, "<h1>Board</h1>");
    assert!(!unpinned.pinned);
    assert_eq!(after_unpinning.version, Version { major: 1, minor: 1 });
    assert!(!after_unpinning.pinned);
    assert!(!updated.pinned, "taking a version unpins the install");
}

#[tokio::test]
async fn a_copy_of_an_installed_page_is_a_new_document_of_the_version_shown() {
    let (service, pages, _, documents, _) = marketplace_with_documents(FakeNotifier::default());
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
        .expect("Ben installs the board");
    pages.save(BOARD, "<h1>Board, tidied</h1>");
    service
        .publish_version(
            &user(ADA),
            Some(&ada),
            listing.id,
            owned(ADA, BOARD),
            VersionBump::Minor,
            Changelog::default(),
        )
        .await
        .expect("a minor version publishes");

    let copy = service
        .copy(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("Ben copies his install");
    let not_installed = service
        .copy(
            &user(CAM),
            Some(&member(CAM, ACME, TeamRole::Member)),
            listing.id,
        )
        .await;
    let still_installed = service
        .installed_build(&user(BEN), Some(&ben), listing.id)
        .await
        .expect("the install stays");

    assert_eq!(copy.kind, ListingKind::Artifact);
    assert_eq!(
        documents.get(copy.id),
        (
            BEN.to_owned(),
            "Board".to_owned(),
            "<h1>Board, tidied</h1>".to_owned()
        )
    );
    assert!(matches!(not_installed, Err(MarketplaceError::NotFound)));
    assert_eq!(still_installed.version, Version { major: 1, minor: 1 });
}

#[tokio::test]
async fn publishing_a_copy_says_what_it_is_based_on_to_those_who_see_the_origin() {
    let (service, pages) = marketplace();
    let origin = published_board(&service, &pages).await;
    let ada = member(ADA, ACME, TeamRole::Member);
    let ben = member(BEN, ACME, TeamRole::Member);
    let cam = member(CAM, ACME, TeamRole::Member);
    service
        .install(&user(BEN), Some(&ben), origin.id, InstallRequest::default())
        .await
        .expect("Ben installs the board");
    let copy = service
        .copy(&user(BEN), Some(&ben), origin.id)
        .await
        .expect("Ben copies his install");
    pages.save(copy.id, "<h1>Ben's board</h1>");

    let bens = service
        .publish(
            &user(BEN),
            Some(&ben),
            owned(BEN, copy.id),
            text("Ben's board"),
            PublishScope::Team,
            Changelog::default(),
        )
        .await
        .expect("Ben publishes his copy");
    let for_cam = service
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .page
        .listings;
    // Unlisted, the board is seen only by Ben, who installed it.
    service
        .unlist(&user(ADA), Some(&ada), origin.id)
        .await
        .expect("Ada unlists the board");
    let cams_view = service
        .listings(&user(CAM), Some(&cam), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .page
        .listings;
    let bens_view = service
        .listings(&user(BEN), Some(&ben), first_page(ListingsShown::Listed))
        .await
        .expect("listings load")
        .page
        .listings;

    let origin_seen = Some(ListingOrigin {
        id: origin.id,
        title: "Board".to_owned(),
    });
    assert_eq!(bens.based_on, origin_seen);
    let cams_copy = for_cam.iter().find(|listing| listing.id == bens.id);
    assert_eq!(
        cams_copy.and_then(|listing| listing.based_on.clone()),
        origin_seen
    );
    assert_eq!(cams_view.len(), 1);
    assert_eq!(cams_view[0].based_on, None);
    assert_eq!(bens_view[0].based_on, origin_seen);
}
