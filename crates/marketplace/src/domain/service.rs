//! The marketplace service: every rule about who may publish, see, install
//! and run a listing.

#[cfg(test)]
mod test;

use std::sync::Arc;

use agent_code_mode::domain::{BuildOutcome, RunProgram, UserExecutionReceipt};
use entity_access::domain::models::{
    EntityAccessReceipt, EntityType, MemberTeamRole, OwnerAccessLevel, TeamRole, ViewAccessLevel,
};
use macro_user_id::user_id::MacroUserIdStr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::models::{
    AddedVersion, InstalledBuild, InstalledPage, Listing, ListingId, ListingKind, ListingText,
    MarketplaceError, NewListing, PublishedPage, VersionBump,
};
use crate::domain::ports::{ListingRepo, MarketplaceService, Pages, Rollout};

/// The marketplace service over its repository, page builder and rollout.
pub struct MarketplaceServiceImpl<Listings, P, R> {
    listings: Listings,
    pages: Arc<P>,
    rollout: R,
}

impl<Listings, P, R> MarketplaceServiceImpl<Listings, P, R>
where
    Listings: ListingRepo,
    P: Pages,
    R: Rollout,
{
    /// Bind the repository, the page builder and the rollout.
    pub fn new(listings: Listings, pages: Arc<P>, rollout: R) -> Self {
        Self {
            listings,
            pages,
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

/// The id of a document the caller's receipt proves they own.
fn owned_document(
    user: &MacroUserIdStr<'static>,
    document: &EntityAccessReceipt<OwnerAccessLevel>,
) -> Result<Uuid, MarketplaceError> {
    let same_user = document
        .get_authenticated_user()
        .is_ok_and(|owner| owner == user);
    if !same_user || document.entity().entity_type != EntityType::Document {
        return Err(MarketplaceError::Forbidden);
    }

    Uuid::parse_str(&document.entity().entity_id)
        .map_err(|_| MarketplaceError::Invalid("this document can't be published".to_owned()))
}

fn internal<E: std::error::Error + Send + Sync + 'static>(error: E) -> MarketplaceError {
    MarketplaceError::Internal(rootcause::Report::new(error).into_dynamic())
}

impl<Listings, P, R> MarketplaceService for MarketplaceServiceImpl<Listings, P, R>
where
    Listings: ListingRepo,
    P: Pages,
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
            source_document_id = tracing::field::Empty,
        )
    )]
    async fn publish(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        document: EntityAccessReceipt<OwnerAccessLevel>,
        text: ListingText,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let team_id = membership(user, team)?
            .ok_or(MarketplaceError::NoTeam)?
            .team_id;
        let text = text.normalized()?;
        let source_document_id = owned_document(user, &document)?;
        let span = tracing::Span::current();
        span.record(
            "source_document_id",
            tracing::field::display(source_document_id),
        );
        let page = self.published_page(document).await?;

        let id = ListingId::generate();
        span.record("listing_id", tracing::field::display(id));
        let created = self
            .listings
            .create_listing(NewListing {
                id,
                kind: ListingKind::Artifact,
                publisher: user.clone(),
                team_id,
                source_document_id,
                text,
                page,
            })
            .await
            .map_err(internal)?;
        if !created {
            return Err(MarketplaceError::AlreadyPublished);
        }

        self.listing(user, Some(team_id), id).await
    }

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn publish_version(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
        document: EntityAccessReceipt<OwnerAccessLevel>,
        bump: VersionBump,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let team = membership(user, team)?.map(|member| member.team_id);
        let listing = self.listing(user, team, id).await?;
        if listing.publisher != *user {
            return Err(MarketplaceError::Forbidden);
        }
        if owned_document(user, &document)? != listing.source_document_id {
            return Err(MarketplaceError::Invalid(
                "a listing's versions come from the page it was published from".to_owned(),
            ));
        }

        let page = self.published_page(document).await?;
        let added = self
            .listings
            .add_version(id, bump, &page)
            .await
            .map_err(internal)?;
        match added {
            AddedVersion::Added(_) => self.listing(user, team, id).await,
            AddedVersion::Unchanged => Err(MarketplaceError::Unchanged),
            AddedVersion::Missing => Err(MarketplaceError::NotFound),
        }
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

    #[tracing::instrument(err, skip_all, fields(listing_id = %id, team_id = tracing::field::Empty))]
    async fn install(
        &self,
        user: &MacroUserIdStr<'static>,
        team: Option<&EntityAccessReceipt<MemberTeamRole>>,
        id: ListingId,
    ) -> Result<Listing, MarketplaceError> {
        self.admit(user).await?;
        let team = membership(user, team)?.map(|member| member.team_id);
        // Seeing it is what allows installing it; an unlisted listing is
        // seen only by those who already did.
        self.listing(user, team, id).await?;

        self.listings
            .install_latest(id, user)
            .await
            .map_err(internal)?;
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
