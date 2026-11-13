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
use scheduled_action::domain::templates::{RoutineSnapshot, fill::RoutineFills};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use uuid::Uuid;

use crate::domain::models::{
    AddedVersion, FollowingInstall, InstallRequest, InstallUpdate, InstalledBuild, InstalledPage,
    Listing, ListingContent, ListingId, ListingKind, ListingText, MarketplaceError, NewListing,
    PublishSource, PublishedPage, PublishedRoutine, PublishedVersion, RoutineCopy, RoutineInstall,
    RoutineUpdate, RoutineUpdates, RoutineVersion, Version, VersionBump,
};
use crate::domain::ports::{ListingRepo, MarketplaceService, Pages, Rollout, Routines};

/// How many installed routines a minor version updates at once.
const UPDATE_CONCURRENCY: usize = 8;

/// The marketplace service over its repository, page builder, routines and
/// rollout.
pub struct MarketplaceServiceImpl<Listings, P, Ro, R> {
    listings: Arc<Listings>,
    pages: Arc<P>,
    routines: Arc<Ro>,
    rollout: R,
}

impl<Listings, P, Ro, R> MarketplaceServiceImpl<Listings, P, Ro, R>
where
    Listings: ListingRepo,
    P: Pages,
    Ro: Routines,
    R: Rollout,
{
    /// Bind the repository, the page builder, routines and the rollout.
    pub fn new(listings: Listings, pages: Arc<P>, routines: Arc<Ro>, rollout: R) -> Self {
        Self {
            listings: Arc::new(listings),
            pages,
            routines,
            rollout,
        }
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
        team: Option<Uuid>,
        id: ListingId,
    ) -> Result<Listing, MarketplaceError> {
        self.listings
            .listings(user, team, Some(id))
            .await
            .map_err(internal)?
            .into_iter()
            .next()
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

    /// Fill the listing's newest routine version in as the caller's own and
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

        let updated = match &existing {
            Some(install) => {
                self.routines
                    .update(user, install.routine_id, &filled, None)
                    .await?
            }
            None => RoutineUpdate::Gone,
        };
        let copy = match (updated, existing) {
            (RoutineUpdate::Updated(task_hash), Some(install)) => RoutineCopy {
                routine_id: install.routine_id,
                task_hash,
            },
            // No routine yet, or the one it made is gone: make one.
            _ => self.routines.create(user, &filled, request.enabled).await?,
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

impl<Listings, P, Ro, R> MarketplaceService for MarketplaceServiceImpl<Listings, P, Ro, R>
where
    Listings: ListingRepo,
    P: Pages,
    Ro: Routines,
    R: Rollout,
{
    #[tracing::instrument(err, skip_all, fields(team_id = tracing::field::Empty))]
    async fn listings(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
    ) -> Result<Vec<Listing>, MarketplaceError> {
        self.admit(user).await?;
        let team = membership(user, team)?.map(|member| member.team_id);
        self.listings
            .listings(user, team, None)
            .await
            .map_err(internal)
    }

    #[tracing::instrument(
        err,
        skip_all,
        fields(
            team_id = tracing::field::Empty,
            listing_id = tracing::field::Empty,
            source_id = tracing::field::Empty,
            kind = %source.kind(),
        )
    )]
    async fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        source: PublishSource,
        text: ListingText,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let team_id = membership(user, team)?
            .ok_or(MarketplaceError::NoTeam)?
            .team_id;
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
            })
            .await
            .map_err(internal)?;
        if !created {
            return Err(MarketplaceError::AlreadyPublished);
        }

        self.listing(user, Some(team_id), id).await
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
        let team = membership(user, team)?.map(|member| member.team_id);
        let listing = self.listing(user, team, id).await?;
        if listing.publisher != *user {
            return Err(MarketplaceError::Forbidden);
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
            .add_version(id, bump, &content)
            .await
            .map_err(internal)?;
        let version = match added {
            AddedVersion::Added(version) => version,
            AddedVersion::Unchanged => return Err(MarketplaceError::Unchanged),
            AddedVersion::Missing => return Err(MarketplaceError::NotFound),
        };

        // The version is stored: updating installs can't fail the publish.
        let updates = match (content, bump) {
            (ListingContent::Routine(routine), VersionBump::Minor) => {
                self.update_installs(id, version, routine.snapshot).await
            }
            _ => RoutineUpdates::default(),
        };

        Ok(PublishedVersion {
            listing: self.listing(user, team, id).await?,
            bump,
            updates,
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
        let team = membership(user, team)?.map(|member| member.team_id);
        let listing = self.listing(user, team, id).await?;
        if listing.kind != ListingKind::Routine {
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
        let team = member.as_ref().map(|member| member.team_id);
        let listing = self.listing(user, team, id).await?;

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
        let team = membership(user, team)?.map(|member| member.team_id);
        // Seeing it is what allows installing it; an unlisted listing is
        // seen only by those who already did.
        let listing = self.listing(user, team, id).await?;

        match listing.kind {
            ListingKind::Artifact => self
                .listings
                .install_latest(id, user)
                .await
                .map_err(internal)?,
            ListingKind::Routine => self.install_routine(user, id, request).await?,
        }
        self.listing(user, team, id).await
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
}
