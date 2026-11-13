//! The marketplace service: every rule about who may publish, see, install
//! and run a listing.

#[cfg(test)]
mod test;

use std::sync::Arc;

use agent_code_mode::domain::{BuildOutcome, RunProgram, UserExecutionReceipt};
use entity_access::domain::models::{
    EntityAccessReceipt, EntityType, MemberTeamRole, OwnerAccessLevel, TeamRole, ViewAccessLevel,
};
use futures::StreamExt;
use macro_user_id::user_id::MacroUserIdStr;
use scheduled_action::domain::templates::{
    RoutineSnapshot,
    fill::{FilledRoutine, RoutineFills},
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use uuid::Uuid;

use crate::domain::models::{
    AddedVersion, Audience, FollowingInstall, InstallRequest, InstallUpdate, InstalledBuild,
    InstalledPage, Listing, ListingContent, ListingId, ListingKind, ListingQuery, ListingScope,
    ListingText, MAX_NOTIFIED_INSTALLS, MarketplaceError, NewListing, PendingVersion, PublishScope,
    PublishSource, PublishedPage, PublishedRoutine, PublishedVersion, ReviewDecision,
    ReviewOutcome, ReviewPreview, ReviewState, RoutineCopy, RoutineInstall, RoutineUpdate,
    RoutineUpdates, RoutineVersion, Version, VersionBump, VersionNotice, ViewerListings,
    interface_changes,
};
use crate::domain::ports::{
    ListingRepo, MarketplaceService, Pages, Rollout, Routines, VersionNotifier,
};

/// How many installed routines a minor version updates at once.
const UPDATE_CONCURRENCY: usize = 8;

/// How long a first routine install's claim holds without a routine before
/// another install takes it over: well past a routine's save, which checks
/// its code.
const CLAIM_STALE_AFTER_SECS: u32 = 120;

/// Most pending versions the review list shows.
const MAX_PENDING_VERSIONS: u16 = 100;

/// The marketplace service over its repository, page builder, routines,
/// rollout and version notifier.
pub struct MarketplaceServiceImpl<Listings, P, Ro, R, N> {
    listings: Arc<Listings>,
    pages: Arc<P>,
    routines: Arc<Ro>,
    rollout: R,
    notifier: N,
    /// Macro's publisher team, whose members publish to everyone and review
    /// each other's versions. Without one, nobody publishes to everyone.
    publisher_team: Option<Uuid>,
}

impl<Listings, P, Ro, R, N> MarketplaceServiceImpl<Listings, P, Ro, R, N>
where
    Listings: ListingRepo,
    P: Pages,
    Ro: Routines,
    R: Rollout,
    N: VersionNotifier,
{
    /// Bind the repository, the page builder, routines, the rollout and the
    /// notifier.
    pub fn new(
        listings: Listings,
        pages: Arc<P>,
        routines: Arc<Ro>,
        rollout: R,
        notifier: N,
    ) -> Self {
        Self {
            listings: Arc::new(listings),
            pages,
            routines,
            rollout,
            notifier,
            publisher_team: None,
        }
    }

    /// Let the members of `team` publish to everyone and review each other's
    /// versions. `None` lets nobody.
    pub fn with_publisher_team(mut self, team: Option<Uuid>) -> Self {
        self.publisher_team = team;
        self
    }

    /// Who the caller is, as listing visibility needs it.
    fn audience(&self, member: Option<&Membership>) -> Audience {
        let team = member.map(|member| member.team_id);
        Audience {
            team,
            global_publisher: team.is_some() && team == self.publisher_team,
        }
    }

    /// The caller's audience, refused unless they are a global publisher.
    fn global_publisher(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
    ) -> Result<Audience, MarketplaceError> {
        let audience = self.audience(membership(user, team)?.as_ref());
        if audience.global_publisher {
            Ok(audience)
        } else {
            Err(MarketplaceError::NotGlobalPublisher)
        }
    }

    /// A version that just became live, for everyone who sees its listing:
    /// a minor routine version updates the installs on its major, then its
    /// installers are told.
    async fn go_live(
        &self,
        id: ListingId,
        version: Version,
        content: ListingContent,
    ) -> RoutineUpdates {
        let updates = match content {
            ListingContent::Routine(routine) if version.minor > 0 => {
                self.update_installs(id, version, routine.snapshot).await
            }
            _ => RoutineUpdates::default(),
        };
        // After the updates, so a list the notice refetches shows them.
        self.notify_installers(id, version).await;
        updates
    }

    /// Tell the installers a new version concerns, so their open pages and
    /// lists refetch: those on its major for a minor version, which their
    /// pages show at once, and everyone for a major one, which they may take.
    /// The version is already stored, so a failure here is only logged.
    #[tracing::instrument(
        skip_all,
        fields(listing_id = %id, recipient_count = tracing::field::Empty, truncated = tracing::field::Empty)
    )]
    async fn notify_installers(&self, id: ListingId, version: Version) {
        // The stored version, not the bump asked for, says which it is: a
        // major version starts at minor 0.
        let major = (version.minor > 0).then_some(version.major);
        // One more than the bound, to tell a listing at the bound from one
        // past it.
        let Ok(mut installers) = self
            .listings
            .installers(id, major, MAX_NOTIFIED_INSTALLS.saturating_add(1))
            .await
            .inspect_err(|error| {
                tracing::error!(error = ?error, "failed to read a new version's installers");
            })
        else {
            return;
        };

        let truncated = installers.len() > usize::from(MAX_NOTIFIED_INSTALLS);
        installers.truncate(usize::from(MAX_NOTIFIED_INSTALLS));
        let span = tracing::Span::current();
        span.record("recipient_count", installers.len());
        span.record("truncated", truncated);
        if installers.is_empty() {
            return;
        }

        let notice = VersionNotice {
            listing_id: id,
            version,
        };
        let _ = self
            .notifier
            .notify(notice, &installers)
            .await
            .inspect_err(|error| {
                tracing::error!(error = ?error, "failed to notify a new version's installers");
            });
    }

    /// A caller outside the rollout sees no marketplace at all.
    async fn admit(&self, user: &MacroUserIdStr<'static>) -> Result<(), MarketplaceError> {
        if self.rollout.allows(user).await {
            Ok(())
        } else {
            Err(MarketplaceError::NotFound)
        }
    }

    /// One listing the caller may see.
    async fn listing(
        &self,
        user: &MacroUserIdStr<'static>,
        audience: Audience,
        id: ListingId,
    ) -> Result<Listing, MarketplaceError> {
        self.listings
            .listing(user, audience, id)
            .await
            .map_err(internal)?
            .ok_or(MarketplaceError::NotFound)
    }

    /// The document's current source, as a version stores it. A page with
    /// problems is refused, so no install ever shows one.
    async fn published_page(
        &self,
        document: EntityAccessReceipt<OwnerAccessLevel>,
    ) -> Result<PublishedPage, MarketplaceError> {
        let document = document
            .try_into_requirement::<ViewAccessLevel>()
            .map_err(|_| MarketplaceError::Forbidden)?;
        let html = self.pages.source(&document).await?;
        let build = self.pages.build(&html).await?;
        if matches!(*build.outcome, BuildOutcome::Failed(_)) {
            return Err(MarketplaceError::DoesNotBuild);
        }

        Ok(PublishedPage {
            source_hash: build.source_hash,
            html,
        })
    }

    /// What a new version of `source` holds: a document's page, or a
    /// routine's snapshot described as `description`.
    async fn content(
        &self,
        source: PublishSource,
        description: &str,
    ) -> Result<ListingContent, MarketplaceError> {
        match source {
            PublishSource::Artifact(document) => {
                Ok(ListingContent::Page(self.published_page(document).await?))
            }
            PublishSource::Routine { routine, variables } => {
                let snapshot = self
                    .routines
                    .snapshot(&routine, description, variables)
                    .await?;
                Ok(ListingContent::Routine(PublishedRoutine::new(snapshot)))
            }
        }
    }

    /// Fill the listing's newest approved routine version in as the caller's own and
    /// save it: a new routine, or the one their install made, which then
    /// follows minor versions again even if they had edited it.
    async fn install_routine(
        &self,
        user: &MacroUserIdStr<'static>,
        id: ListingId,
        request: InstallRequest,
    ) -> Result<(), MarketplaceError> {
        let newest = self
            .listings
            .routine_version(id, None)
            .await
            .map_err(internal)?
            .ok_or(MarketplaceError::NotFound)?;
        let existing = self
            .listings
            .routine_install(id, user)
            .await
            .map_err(internal)?;

        // A new major is filled in from the fills stored when installing,
        // with the caller's new ones on top.
        let fills = match (&existing, request.timezone) {
            (Some(install), timezone) => install.fills.clone().merged(RoutineFills {
                timezone: timezone.unwrap_or(install.fills.timezone),
                variables: request.variables,
            }),
            (None, Some(timezone)) => RoutineFills {
                timezone,
                variables: request.variables,
            },
            (None, None) => {
                return Err(MarketplaceError::Invalid(
                    "installing a routine needs your time zone".to_owned(),
                ));
            }
        };
        let filled = newest
            .snapshot
            .fill(user, &fills)
            .map_err(|missing| MarketplaceError::MissingFills(missing.0))?;

        let copy = match existing {
            Some(install) => {
                match self
                    .routines
                    .update(user, install.routine_id, &filled, None)
                    .await?
                {
                    RoutineUpdate::Updated(task_hash) => RoutineCopy {
                        routine_id: install.routine_id,
                        task_hash,
                    },
                    // The routine it made is gone: make another.
                    RoutineUpdate::Edited | RoutineUpdate::Gone => {
                        self.routines.create(user, &filled, request.enabled).await?
                    }
                }
            }
            None => {
                self.create_claimed(user, id, newest.version.major, &filled, request.enabled)
                    .await?
            }
        };
        tracing::Span::current().record("routine_id", tracing::field::display(copy.routine_id));

        self.listings
            .save_routine_install(
                id,
                user,
                &RoutineInstall {
                    version: newest.version,
                    routine_id: copy.routine_id,
                    fills,
                    task_hash: copy.task_hash,
                },
            )
            .await
            .map_err(internal)
    }

    /// A first install's routine, created only by the request that claims
    /// the install, so two installs at once make one routine. A claim whose
    /// routine fails to save is released for the next install to try; one
    /// left by a stopped process goes stale and is taken over.
    async fn create_claimed(
        &self,
        user: &MacroUserIdStr<'static>,
        id: ListingId,
        major: i32,
        filled: &FilledRoutine,
        enabled: bool,
    ) -> Result<RoutineCopy, MarketplaceError> {
        let claimed = self
            .listings
            .claim_routine_install(id, user, major, CLAIM_STALE_AFTER_SECS)
            .await
            .map_err(internal)?;
        if !claimed {
            return Err(MarketplaceError::InstallInProgress);
        }

        let created = self.routines.create(user, filled, enabled).await;
        if created.is_err()
            && let Err(error) = self.listings.release_routine_claim(id, user).await
        {
            tracing::error!(error = %error, "could not release an install claim");
        }
        created
    }

    /// Bring every install on `version`'s major that still follows the
    /// listing to `version`. A failed update is recorded on its install and
    /// fails nothing else; the next minor version tries it again, as does
    /// its person taking the listing again.
    ///
    /// The updates run on their own task, so a publish request dropped
    /// mid-way can't leave a routine updated and its install unrecorded,
    /// which would read as its person's edit.
    async fn update_installs(
        &self,
        id: ListingId,
        version: Version,
        snapshot: RoutineSnapshot,
    ) -> RoutineUpdates {
        let installs = match self.listings.installs_behind(id, version).await {
            Ok(installs) => installs,
            Err(error) => {
                tracing::error!(error = %error, "could not read the installs to update");
                return RoutineUpdates::default();
            }
        };

        let listings = Arc::clone(&self.listings);
        let routines = Arc::clone(&self.routines);
        let updates = async move {
            let snapshot = &snapshot;
            futures::stream::iter(installs)
                .map(|following| {
                    update_install(&*listings, &*routines, id, version, snapshot, following)
                })
                .buffer_unordered(UPDATE_CONCURRENCY)
                .fold(
                    RoutineUpdates::default(),
                    |mut updates, update| async move {
                        match update {
                            InstallUpdate::Updated { .. } => updates.updated += 1,
                            InstallUpdate::Detached => updates.detached += 1,
                            InstallUpdate::Failed => updates.failed += 1,
                        }
                        updates
                    },
                )
                .await
        };

        let updates = tokio::spawn(updates.in_current_span())
            .await
            .unwrap_or_else(|error| {
                tracing::error!(error = %error, "updating installed routines stopped");
                RoutineUpdates::default()
            });
        let span = tracing::Span::current();
        span.record("routines_updated", updates.updated);
        span.record("routines_detached", updates.detached);
        span.record("routines_failed", updates.failed);
        updates
    }

    /// The page the caller's install shows.
    async fn installed_page(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<InstalledPage, MarketplaceError> {
        self.admit(user).await?;
        let team = membership(user, team)?.map(|member| member.team_id);
        self.listings
            .installed_page(id, user, team)
            .await
            .map_err(internal)?
            .ok_or(MarketplaceError::NotFound)
    }
}

/// The caller's team, read from their membership receipt.
struct Membership {
    team_id: Uuid,
    admin: bool,
}

/// The caller's membership, when they have a team, recorded as `team_id` on
/// the current span. A receipt for someone else, or for something that isn't
/// a team, is refused.
fn membership(
    user: &MacroUserIdStr<'static>,
    team: Option<&EntityAccessReceipt<MemberTeamRole>>,
) -> Result<Option<Membership>, MarketplaceError> {
    let Some(team) = team else {
        return Ok(None);
    };

    let same_user = team
        .get_authenticated_user()
        .is_ok_and(|member| member == user);
    if !same_user || team.entity().entity_type != EntityType::Team {
        return Err(MarketplaceError::Forbidden);
    }

    let team_id =
        Uuid::parse_str(&team.entity().entity_id).map_err(|_| MarketplaceError::Forbidden)?;
    tracing::Span::current().record("team_id", tracing::field::display(team_id));
    Ok(Some(Membership {
        team_id,
        admin: team.entity_permission().allows_team_role(TeamRole::Admin),
    }))
}

/// The id of the document or routine the caller's receipt proves they own,
/// as a listing of `kind` publishes it.
fn owned_source(
    user: &MacroUserIdStr<'static>,
    source: &EntityAccessReceipt<OwnerAccessLevel>,
    kind: ListingKind,
) -> Result<Uuid, MarketplaceError> {
    let entity_type = match kind {
        ListingKind::Artifact => EntityType::Document,
        ListingKind::Routine => EntityType::ScheduledAction,
    };
    let same_user = source
        .get_authenticated_user()
        .is_ok_and(|owner| owner == user);
    if !same_user || source.entity().entity_type != entity_type {
        return Err(MarketplaceError::Forbidden);
    }

    Uuid::parse_str(&source.entity().entity_id)
        .map_err(|_| MarketplaceError::Invalid("this can't be published".to_owned()))
}

/// The bump a routine version gets: what was asked, unless the routine asks
/// something new of those who installed it, which only a major version may.
fn routine_bump(
    requested: VersionBump,
    newest: &RoutineSnapshot,
    next: &RoutineSnapshot,
) -> VersionBump {
    if newest.interface() == next.interface() {
        requested
    } else {
        VersionBump::Major
    }
}

/// Update one install's routine to `version`, unless its person edited it,
/// and record what came of it.
async fn update_install<Listings: ListingRepo, Ro: Routines>(
    listings: &Listings,
    routines: &Ro,
    id: ListingId,
    version: Version,
    snapshot: &RoutineSnapshot,
    following: FollowingInstall,
) -> InstallUpdate {
    let FollowingInstall { user, install } = following;
    let saved = match snapshot.fill(&user, &install.fills) {
        Ok(filled) => {
            routines
                .update(&user, install.routine_id, &filled, Some(&install.task_hash))
                .await
        }
        Err(missing) => Err(MarketplaceError::MissingFills(missing.0)),
    };

    // Only an edit detaches. A routine that can't be found is a failure, not
    // a decision of its person's: it may be a passing access error, and a
    // routine deleted for good keeps failing until its person uninstalls or
    // installs again, which makes a new one.
    let update = match saved {
        Ok(RoutineUpdate::Updated(task_hash)) => InstallUpdate::Updated { version, task_hash },
        Ok(RoutineUpdate::Edited) => InstallUpdate::Detached,
        Ok(RoutineUpdate::Gone) => {
            tracing::warn!(
                routine_id = %install.routine_id,
                "an installed routine to update was not found"
            );
            InstallUpdate::Failed
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                routine_id = %install.routine_id,
                "could not update an installed routine"
            );
            InstallUpdate::Failed
        }
    };

    if let Err(error) = listings.record_update(id, &user, &update).await {
        tracing::error!(
            error = %error,
            routine_id = %install.routine_id,
            "could not record an installed routine's update"
        );
    }
    update
}

fn internal<E: std::error::Error + Send + Sync + 'static>(error: E) -> MarketplaceError {
    MarketplaceError::Internal(rootcause::Report::new(error).into_dynamic())
}

impl<Listings, P, Ro, R, N> MarketplaceService for MarketplaceServiceImpl<Listings, P, Ro, R, N>
where
    Listings: ListingRepo,
    P: Pages,
    Ro: Routines,
    R: Rollout,
    N: VersionNotifier,
{
    #[tracing::instrument(err, skip_all, fields(team_id = tracing::field::Empty))]
    async fn listings(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        query: ListingQuery,
    ) -> Result<ViewerListings, MarketplaceError> {
        self.admit(user).await?;
        let audience = self.audience(membership(user, team)?.as_ref());
        let page = self
            .listings
            .listings(user, audience, &query)
            .await
            .map_err(internal)?;
        Ok(ViewerListings {
            page,
            global_publisher: audience.global_publisher,
        })
    }

    #[tracing::instrument(
        err,
        skip_all,
        fields(
            team_id = tracing::field::Empty,
            listing_id = tracing::field::Empty,
            source_id = tracing::field::Empty,
            kind = %source.kind(),
            scope = ?scope,
        )
    )]
    async fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        source: PublishSource,
        text: ListingText,
        scope: PublishScope,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let member = membership(user, team)?.ok_or(MarketplaceError::NoTeam)?;
        let audience = self.audience(Some(&member));
        // A global listing's first version waits for a second publisher.
        let (team_id, review) = match scope {
            PublishScope::Team => (Some(member.team_id), ReviewState::Approved),
            PublishScope::Global if audience.global_publisher => (None, ReviewState::Pending),
            PublishScope::Global => return Err(MarketplaceError::NotGlobalPublisher),
        };
        let text = text.normalized()?;
        let source_id = owned_source(user, source.receipt(), source.kind())?;
        let span = tracing::Span::current();
        span.record("source_id", tracing::field::display(source_id));
        let content = self.content(source, &text.description).await?;

        let id = ListingId::generate();
        span.record("listing_id", tracing::field::display(id));
        let created = self
            .listings
            .create_listing(NewListing {
                id,
                publisher: user.clone(),
                team_id,
                source_id,
                text,
                content,
                review,
            })
            .await
            .map_err(internal)?;
        if !created {
            return Err(MarketplaceError::AlreadyPublished);
        }

        self.listing(user, audience, id).await
    }

    #[tracing::instrument(
        err,
        skip_all,
        fields(
            listing_id = %id,
            team_id = tracing::field::Empty,
            bump = tracing::field::Empty,
            routines_updated = tracing::field::Empty,
            routines_detached = tracing::field::Empty,
            routines_failed = tracing::field::Empty,
        )
    )]
    async fn publish_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        source: PublishSource,
        bump: VersionBump,
    ) -> Result<PublishedVersion, MarketplaceError> {
        self.admit(user).await?;
        let audience = self.audience(membership(user, team)?.as_ref());
        let listing = self.listing(user, audience, id).await?;
        if listing.publisher != *user {
            return Err(MarketplaceError::Forbidden);
        }
        // A global listing's versions are reviewed, and published only by
        // someone still on the publisher team.
        let review = match listing.scope {
            ListingScope::Team { .. } => ReviewState::Approved,
            ListingScope::Global if audience.global_publisher => ReviewState::Pending,
            ListingScope::Global => return Err(MarketplaceError::NotGlobalPublisher),
        };
        if listing.pending_version.is_some() {
            return Err(MarketplaceError::ReviewPending);
        }
        let same_source = source.kind() == listing.kind
            && owned_source(user, source.receipt(), source.kind())? == listing.source_id;
        if !same_source {
            return Err(MarketplaceError::Invalid(
                "a listing's versions come from what it was published from".to_owned(),
            ));
        }

        let content = self.content(source, &listing.description).await?;
        let bump = match &content {
            ListingContent::Page(_) => bump,
            // Against what installs have: the newest approved version.
            ListingContent::Routine(routine) => {
                let newest = self
                    .listings
                    .routine_version(id, None)
                    .await
                    .map_err(internal)?
                    .ok_or(MarketplaceError::NotFound)?;
                routine_bump(bump, &newest.snapshot, &routine.snapshot)
            }
        };
        tracing::Span::current().record("bump", tracing::field::debug(bump));

        let added = self
            .listings
            .add_version(id, bump, &content, review)
            .await
            .map_err(internal)?;
        let version = match added {
            AddedVersion::Added(version) => version,
            AddedVersion::Unchanged => return Err(MarketplaceError::Unchanged),
            AddedVersion::ReviewPending => return Err(MarketplaceError::ReviewPending),
            AddedVersion::Missing => return Err(MarketplaceError::NotFound),
        };

        // The version is stored: updating installs can't fail the publish. A
        // pending one goes live when it is approved.
        let updates = match review {
            ReviewState::Approved => self.go_live(id, version, content).await,
            ReviewState::Pending | ReviewState::Rejected => RoutineUpdates::default(),
        };

        Ok(PublishedVersion {
            listing: self.listing(user, audience, id).await?,
            bump,
            updates,
            review,
        })
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn routine_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<RoutineVersion, MarketplaceError> {
        self.admit(user).await?;
        let audience = self.audience(membership(user, team)?.as_ref());
        let listing = self.listing(user, audience, id).await?;
        if listing.kind != ListingKind::Routine || !listing.has_approved_version() {
            return Err(MarketplaceError::NotFound);
        }

        self.listings
            .routine_version(id, None)
            .await
            .map_err(internal)?
            .ok_or(MarketplaceError::NotFound)
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn unlist(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<(), MarketplaceError> {
        self.admit(user).await?;
        let member = membership(user, team)?;
        let audience = self.audience(member.as_ref());
        let listing = self.listing(user, audience, id).await?;

        let publisher = listing.publisher == *user;
        let team_admin = member
            .is_some_and(|member| member.admin && listing.scope.team_id() == Some(member.team_id));
        if !publisher && !team_admin {
            return Err(MarketplaceError::Forbidden);
        }

        self.listings.remove_listing(id).await.map_err(internal)
    }

    #[tracing::instrument(
        err,
        skip_all,
        fields(listing_id = %id, team_id = tracing::field::Empty, routine_id = tracing::field::Empty)
    )]
    async fn install(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        request: InstallRequest,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let audience = self.audience(membership(user, team)?.as_ref());
        // Seeing it is what allows installing it; an unlisted listing is
        // seen only by those who already did. Only an approved version
        // installs.
        let listing = self.listing(user, audience, id).await?;
        if !listing.has_approved_version() {
            return Err(MarketplaceError::NotFound);
        }

        match listing.kind {
            ListingKind::Artifact => self
                .listings
                .install_latest(id, user)
                .await
                .map_err(internal)?,
            ListingKind::Routine => self.install_routine(user, id, request).await?,
        }
        self.listing(user, audience, id).await
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id))]
    async fn uninstall(
        &self,
        user: &MacroUserIdStr<'static>,
        id: ListingId,
    ) -> Result<(), MarketplaceError> {
        self.admit(user).await?;
        self.listings.uninstall(id, user).await.map_err(internal)
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn installed_build(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<InstalledBuild, MarketplaceError> {
        let page = self.installed_page(user, team, id).await?;
        let build = self.pages.build(&page.html).await?;

        Ok(InstalledBuild {
            version: page.version,
            build,
            html: page.html,
        })
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn run_installed(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        request: RunProgram,
        cancel: CancellationToken,
    ) -> Result<UserExecutionReceipt, MarketplaceError> {
        let page = self.installed_page(user, team, id).await?;
        Ok(self.pages.run(user, &page.html, request, cancel).await?)
    }

    #[tracing::instrument(err, skip_all, fields(team_id = tracing::field::Empty))]
    async fn pending_versions(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
    ) -> Result<Vec<PendingVersion>, MarketplaceError> {
        self.admit(user).await?;
        self.global_publisher(user, team)?;
        self.listings
            .pending_versions(MAX_PENDING_VERSIONS)
            .await
            .map_err(internal)
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn review_preview(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<ReviewPreview, MarketplaceError> {
        self.admit(user).await?;
        self.global_publisher(user, team)?;
        let review = self
            .listings
            .pending_review(id)
            .await
            .map_err(internal)?
            .ok_or(MarketplaceError::NotFound)?;

        match review.content {
            // Built, never run: no route runs a pending version's programs.
            ListingContent::Page(page) => Ok(ReviewPreview::Page {
                pending: review.pending,
                build: self.pages.build(&page.html).await?,
                html: page.html,
            }),
            ListingContent::Routine(routine) => {
                let approved = self
                    .listings
                    .routine_version(id, None)
                    .await
                    .map_err(internal)?;
                Ok(ReviewPreview::Routine {
                    pending: review.pending,
                    interface_changes: interface_changes(
                        approved.as_ref().map(|approved| &approved.snapshot),
                        &routine.snapshot,
                    ),
                    snapshot: routine.snapshot,
                })
            }
        }
    }

    #[tracing::instrument(
        err,
        skip_all,
        fields(
            listing_id = %id,
            version = %version,
            outcome = tracing::field::Empty,
            team_id = tracing::field::Empty,
            routines_updated = tracing::field::Empty,
            routines_detached = tracing::field::Empty,
            routines_failed = tracing::field::Empty,
        )
    )]
    async fn review(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        version: Version,
        decision: ReviewDecision,
    ) -> Result<ReviewOutcome, MarketplaceError> {
        self.admit(user).await?;
        self.global_publisher(user, team)?;
        let decision = decision.normalized()?;
        let review = self
            .listings
            .pending_review(id)
            .await
            .map_err(internal)?
            .filter(|review| review.pending.version == version)
            .ok_or(MarketplaceError::NotFound)?;
        if review.pending.publisher == *user {
            return Err(MarketplaceError::OwnVersion);
        }

        let decided = self
            .listings
            .decide_review(id, version, user, &decision)
            .await
            .map_err(internal)?;
        // Another reviewer decided first.
        if !decided {
            return Err(MarketplaceError::NotFound);
        }
        let state = decision.state();
        tracing::Span::current().record("outcome", tracing::field::display(state));

        let updates = match decision {
            ReviewDecision::Approve => self.go_live(id, version, review.content).await,
            ReviewDecision::Reject { .. } => {
                // A new listing nobody may see goes, so its source can be
                // published again.
                if review.pending.approved_version.is_none() {
                    self.listings.remove_listing(id).await.map_err(internal)?;
                }
                RoutineUpdates::default()
            }
        };

        Ok(ReviewOutcome { state, updates })
    }
}
