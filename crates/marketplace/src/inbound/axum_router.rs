//! Axum router for the marketplace.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_code_mode::domain::{
    CodeModeError, DocumentProgramError, RunProgram, UserExecutionReceipt,
};
use agent_code_mode::inbound::document_router::{
    DocumentBuildResponse, DocumentProgramApiError, RunDocumentProgram,
};
use axum::{
    Json, Router,
    extract::{FromRef, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use chrono_tz::Tz;
use entity_access::{
    domain::{
        models::{AccessError, EntityType, MemberTeamRole, OwnerAccessLevel},
        ports::EntityAccessService,
    },
    inbound::axum_extractors::OptionalMacroUserTeamExtractorV2,
};
use macro_authorization::{
    MacroAuthorizationExtractor, MacroAuthorizationService, MacroAuthorizationState, UserOnly,
    is_agent_run,
};
use macro_user_id::user_id::MacroUserIdStr;
use model_error_response::ErrorResponse;
use scheduled_action::domain::templates::{RoutineSnapshot, TemplateVariable, VariableFill};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::domain::{
    models::{
        Changelog, InstallRequest, InstalledBuild, Listing, ListingCopy, ListingId, ListingKind,
        ListingQuery, ListingText, ListingVersion, ListingsShown, MarketplaceError, PublishSource,
        PublishedVersion, Version, VersionBump,
    },
    ports::MarketplaceService,
};

/// Dependencies for the marketplace routes.
pub struct MarketplaceRouterState<S, Access, Auth> {
    service: Arc<S>,
    access: Arc<Access>,
    authorization: MacroAuthorizationState<Auth>,
}

impl<S, Access, Auth> MarketplaceRouterState<S, Access, Auth> {
    /// Bind the service, the access boundary and authorization.
    pub fn new(
        service: Arc<S>,
        access: Arc<Access>,
        authorization: MacroAuthorizationState<Auth>,
    ) -> Self {
        Self {
            service,
            access,
            authorization,
        }
    }
}

impl<S, Access, Auth> Clone for MarketplaceRouterState<S, Access, Auth> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            access: self.access.clone(),
            authorization: self.authorization.clone(),
        }
    }
}

impl<S, Access, Auth> FromRef<MarketplaceRouterState<S, Access, Auth>>
    for MacroAuthorizationState<Auth>
{
    fn from_ref(state: &MarketplaceRouterState<S, Access, Auth>) -> Self {
        state.authorization.clone()
    }
}

impl<S, Access, Auth> FromRef<MarketplaceRouterState<S, Access, Auth>> for Arc<Access> {
    fn from_ref(state: &MarketplaceRouterState<S, Access, Auth>) -> Self {
        state.access.clone()
    }
}

/// Mount at the service root; paths are `/marketplace/...`.
pub fn marketplace_router<S, Access, Auth>(state: MarketplaceRouterState<S, Access, Auth>) -> Router
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    Router::new()
        .route(
            "/marketplace/listings",
            get(list_marketplace_listings::<S, Access, Auth>)
                .post(publish_marketplace_listing::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}",
            delete(unlist_marketplace_listing::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/versions",
            get(list_marketplace_listing_versions::<S, Access, Auth>)
                .post(publish_marketplace_listing_version::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/routine",
            get(get_marketplace_listing_routine::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install",
            put(install_marketplace_listing::<S, Access, Auth>)
                .delete(uninstall_marketplace_listing::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install/copy",
            post(copy_marketplace_install::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install/pin",
            put(pin_marketplace_install::<S, Access, Auth>)
                .delete(unpin_marketplace_install::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install/build",
            get(get_installed_listing_build::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install/code-executions",
            post(run_installed_listing_program::<S, Access, Auth>),
        )
        .with_state(state)
}

/// A page of the listings the caller may see.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceListings {
    /// Their team's and everyone's, newest first.
    pub listings: Vec<Listing>,
    /// Send as `cursor` for the next page; absent on the last page.
    pub next_cursor: Option<String>,
}

/// Which page of listings to read.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ListMarketplaceListingsParams {
    /// Text the title or description holds, ignoring case.
    pub q: Option<String>,
    /// Listings per page, 1 to 100; 50 when absent.
    pub limit: Option<u16>,
    /// The `nextCursor` of the previous page.
    pub cursor: Option<String>,
    /// The caller's installs, unlisted ones included. Otherwise the listings
    /// still listed.
    #[serde(default)]
    pub installed: bool,
    /// Only listings of this kind.
    pub kind: Option<ListingKind>,
    /// Only listings published from this document or routine: with `kind`,
    /// its live listing, which the publish dialog looks up.
    pub source_id: Option<Uuid>,
}

/// What a listing is published from.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ListingSource {
    /// A stored HTML document the caller owns, as an artifact.
    #[serde(rename_all = "camelCase")]
    Artifact {
        /// The document.
        document_id: Uuid,
    },
    /// A routine the caller owns.
    #[serde(rename_all = "camelCase")]
    Routine {
        /// The routine.
        routine_id: Uuid,
        /// Its user-specific parts, marked as when sharing it as a template.
        #[serde(default)]
        variables: Vec<TemplateVariable>,
    },
}

/// Publish a page or routine to the caller's team.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishListing {
    /// What it is published from.
    pub source: ListingSource,
    /// The listing's name.
    pub title: String,
    /// What it does, in a few lines.
    #[serde(default)]
    pub description: String,
    /// What version 1.0 says about itself, at most 2000 characters.
    #[serde(default)]
    pub changelog: Option<String>,
}

/// Publish a listing's source as its next version.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishListingVersion {
    /// What the listing was published from.
    pub source: ListingSource,
    /// Whether installs get it at once (`minor`) or wait (`major`). A
    /// routine that asks something new of its installs takes a major
    /// version whatever is asked.
    pub bump: VersionBump,
    /// What changed, for installers to read: at most 2000 characters.
    #[serde(default)]
    pub changelog: Option<String>,
}

/// A listing's versions.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListingVersions {
    /// Every version, newest first.
    pub versions: Vec<ListingVersion>,
}

/// Install a listing, or take its newest major version. A routine is filled
/// in as the caller's own; an artifact takes no body.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstallListing {
    /// The caller's time zone, which schedules filled in as them run in.
    /// Required to install a routine; afterwards the stored one is kept.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub timezone: Option<Tz>,
    /// How to fill the routine's variables, by key, on top of those stored:
    /// `shared` keeps a reference, `choice` leaves it out. Every variable its
    /// author left to each person needs one.
    #[serde(default)]
    pub variables: BTreeMap<String, VariableFill>,
    /// Whether a new routine starts switched on.
    #[serde(default)]
    pub enabled: bool,
}

/// A routine listing's newest version.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListingRoutine {
    /// Its version.
    pub version: Version,
    /// The routine as each install gets it, before filling in.
    pub routine: RoutineSnapshot,
}

/// The routine's variables that need a fill before it can be installed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingRoutineFills {
    /// What went wrong, for people.
    pub message: String,
    /// The variables to ask for.
    pub missing: Vec<TemplateVariable>,
}

/// The build of the page an install shows.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledListingBuild {
    /// The version built: the newest minor of the major the caller is on,
    /// or the one they pinned.
    pub version: Version,
    /// The caller pinned their install to `version`: a newer minor version
    /// doesn't replace the page.
    pub pinned: bool,
    /// The page to render, or why it doesn't build.
    pub page: DocumentBuildResponse,
    /// The version's source, when `page` is plain and is shown as it is.
    pub plain_html: Option<String>,
}

impl From<InstalledBuild> for InstalledListingBuild {
    fn from(installed: InstalledBuild) -> Self {
        let page = DocumentBuildResponse::from(installed.build);
        let plain_html =
            matches!(page, DocumentBuildResponse::Plain { .. }).then_some(installed.html);

        Self {
            version: installed.version,
            pinned: installed.pinned,
            page,
            plain_html,
        }
    }
}

impl IntoResponse for MarketplaceError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Invalid(_) | Self::RoutineRejected(_) => StatusCode::BAD_REQUEST,
            Self::NoTeam | Self::AlreadyPublished | Self::Unchanged | Self::InstallInProgress => {
                StatusCode::CONFLICT
            }
            Self::DoesNotBuild | Self::MissingFills(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Page(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match self {
            Self::Page(error) => return DocumentProgramApiError::from(error).into_response(),
            Self::MissingFills(missing) => {
                let message = Self::MissingFills(missing.clone()).to_string();
                return (status, Json(MissingRoutineFills { message, missing })).into_response();
            }
            Self::Internal(_) => {
                tracing::error!(error = ?self, "marketplace internal server error");
                "internal server error".to_owned()
            }
            error => error.to_string(),
        };

        (
            status,
            Json(ErrorResponse {
                message: message.into(),
            }),
        )
            .into_response()
    }
}

/// The caller's owner receipt for a document or routine they want to
/// publish. One they can't see, or don't own, answers like one that doesn't
/// exist.
async fn owned_source<S, Access: EntityAccessService, Auth>(
    state: &MarketplaceRouterState<S, Access, Auth>,
    user: &MacroUserIdStr<'static>,
    source: ListingSource,
) -> Result<PublishSource, MarketplaceError> {
    let (id, entity_type) = match &source {
        ListingSource::Artifact { document_id } => (*document_id, EntityType::Document),
        ListingSource::Routine { routine_id, .. } => (*routine_id, EntityType::ScheduledAction),
    };
    let receipt = state
        .access
        .generate_entity_access_receipt::<OwnerAccessLevel>(
            user,
            None,
            &id.to_string(),
            entity_type,
        )
        .await
        .map_err(|error| match error {
            AccessError::Unauthorized
            | AccessError::UnauthorizedWithMessage(_)
            | AccessError::NotFound(_)
            | AccessError::BadRequest(_) => MarketplaceError::NotFound,
            AccessError::Unavailable(_) => MarketplaceError::Page(DocumentProgramError::Busy),
            AccessError::Internal(report) => MarketplaceError::Internal(report),
        })?;

    Ok(match source {
        ListingSource::Artifact { .. } => PublishSource::Artifact(receipt),
        ListingSource::Routine { variables, .. } => PublishSource::Routine {
            routine: receipt,
            variables,
        },
    })
}

type Caller<Auth> = MacroAuthorizationExtractor<Auth, UserOnly>;
type Team<Access, Auth> = OptionalMacroUserTeamExtractorV2<MemberTeamRole, Access, Auth>;

#[utoipa::path(get, path = "/marketplace/listings", tag = "marketplace", operation_id = "list_marketplace_listings",
    params(ListMarketplaceListingsParams),
    responses((status = 200, body = MarketplaceListings), (status = 400, body = ErrorResponse), (status = 401), (status = 404), (status = 500)))]
/// A page of the listings the caller may see, newest first: their team's and
/// everyone's, each with the major version they are on when they installed
/// it. Those still listed, or with `installed=true` the caller's installs.
#[tracing::instrument(err, skip_all, fields(installed = params.installed, searched = params.q.is_some()))]
pub async fn list_marketplace_listings<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    Query(params): Query<ListMarketplaceListingsParams>,
) -> Result<Json<MarketplaceListings>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let shown = if params.installed {
        ListingsShown::Installed
    } else {
        ListingsShown::Listed
    };
    let query = ListingQuery::new(
        shown,
        params.q.as_deref(),
        params.kind,
        params.source_id,
        params.cursor.as_deref(),
        params.limit,
    )?;

    let page = state
        .service
        .listings(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            query,
        )
        .await?;
    Ok(Json(MarketplaceListings {
        listings: page.listings,
        next_cursor: page.next.map(|cursor| cursor.encode()),
    }))
}

#[utoipa::path(post, path = "/marketplace/listings", tag = "marketplace", operation_id = "publish_marketplace_listing",
    request_body = PublishListing,
    responses((status = 201, body = Listing), (status = 400, body = ErrorResponse), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 413), (status = 422, body = ErrorResponse), (status = 500), (status = 503)))]
/// Publish an HTML document or a routine the caller owns to their team's
/// marketplace, as version 1.0. `409` means the caller has no team or the
/// source already has a listing; `422` means the page doesn't build.
#[tracing::instrument(err, skip_all)]
pub async fn publish_marketplace_listing<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    Json(body): Json<PublishListing>,
) -> Result<(StatusCode, Json<Listing>), MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let user = &caller.authorization.macro_user_id;
    let changelog = Changelog::new(body.changelog.as_deref())?;
    let source = owned_source(&state, user, body.source).await?;
    let listing = state
        .service
        .publish(
            user,
            team.entity_access_receipt.as_ref(),
            source,
            ListingText {
                title: body.title,
                description: body.description,
            },
            changelog,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(listing)))
}

#[utoipa::path(post, path = "/marketplace/listings/{id}/versions", tag = "marketplace", operation_id = "publish_marketplace_listing_version",
    params(("id" = Uuid, Path, description = "The listing")),
    request_body = PublishListingVersion,
    responses((status = 201, body = PublishedVersion), (status = 400, body = ErrorResponse), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 413), (status = 422, body = ErrorResponse), (status = 500), (status = 503)))]
/// Publish the listing's source as its next version. A minor version
/// reaches every install on the newest major at once; a major version waits
/// for each person to take it. A routine whose variables, required apps,
/// trigger kinds or knobs changed takes a major version whatever was asked,
/// and the answer says which bump applied. A minor routine version updates
/// each installed routine its person hasn't edited, after the version is
/// stored. `409` means nothing changed.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn publish_marketplace_listing_version<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    Json(body): Json<PublishListingVersion>,
) -> Result<(StatusCode, Json<PublishedVersion>), MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let user = &caller.authorization.macro_user_id;
    let changelog = Changelog::new(body.changelog.as_deref())?;
    let source = owned_source(&state, user, body.source).await?;
    let published = state
        .service
        .publish_version(
            user,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            source,
            body.bump,
            changelog,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(published)))
}

#[utoipa::path(get, path = "/marketplace/listings/{id}/versions", tag = "marketplace", operation_id = "list_marketplace_listing_versions",
    params(("id" = Uuid, Path, description = "The listing")),
    responses((status = 200, body = ListingVersions), (status = 401), (status = 404), (status = 500)))]
/// The listing's versions, newest first: each one's changelog, when it was
/// published, and whether the server made a routine version major. Anyone
/// who sees the listing sees them.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn list_marketplace_listing_versions<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<Json<ListingVersions>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let versions = state
        .service
        .versions(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(Json(ListingVersions { versions }))
}

#[utoipa::path(get, path = "/marketplace/listings/{id}/routine", tag = "marketplace", operation_id = "get_marketplace_listing_routine",
    params(("id" = Uuid, Path, description = "A routine listing")),
    responses((status = 200, body = ListingRoutine), (status = 401), (status = 404), (status = 500)))]
/// A routine listing's newest version, to fill in before installing it or
/// taking its newest major.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn get_marketplace_listing_routine<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<Json<ListingRoutine>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let routine = state
        .service
        .routine_version(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(Json(ListingRoutine {
        version: routine.version,
        routine: routine.snapshot,
    }))
}

#[utoipa::path(delete, path = "/marketplace/listings/{id}", tag = "marketplace", operation_id = "unlist_marketplace_listing",
    params(("id" = Uuid, Path, description = "The listing")),
    responses((status = 204), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 500)))]
/// Unlist a listing: it leaves the marketplace, and those who installed it
/// keep it. Its publisher or an admin of its team may.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn unlist_marketplace_listing<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<StatusCode, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    state
        .service
        .unlist(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/marketplace/listings/{id}/install", tag = "marketplace", operation_id = "install_marketplace_listing",
    params(("id" = Uuid, Path, description = "The listing")),
    request_body(content = Option<InstallListing>, description = "A routine's fills; an artifact takes none"),
    responses((status = 200, body = Listing), (status = 400, body = ErrorResponse), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 422, body = MissingRoutineFills), (status = 500)))]
/// Put the caller on the listing's newest major version: installs it, or
/// takes a major version they were waiting on. Repeating it changes nothing
/// for an artifact.
///
/// A routine listing creates the caller's own routine, filled in from the
/// body and the fills stored when they first installed it; taking a new
/// major fills in their routine again, replacing any edits, and it follows
/// minor versions again. `422` lists the variables that still need a fill;
/// `409` means another request is installing it for the caller right now.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn install_marketplace_listing<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    body: Option<Json<InstallListing>>,
) -> Result<Json<Listing>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let body = body.map(|Json(body)| body).unwrap_or_default();
    let listing = state
        .service
        .install(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            InstallRequest {
                timezone: body.timezone,
                variables: body.variables,
                enabled: body.enabled,
            },
        )
        .await?;
    Ok(Json(listing))
}

#[utoipa::path(delete, path = "/marketplace/listings/{id}/install", tag = "marketplace", operation_id = "uninstall_marketplace_listing",
    params(("id" = Uuid, Path, description = "The listing")),
    responses((status = 204), (status = 401), (status = 404, body = ErrorResponse), (status = 500)))]
/// Remove the caller's install. Repeating it changes nothing. A routine the
/// install created stays the caller's own and no longer follows the listing.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn uninstall_marketplace_listing<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
) -> Result<StatusCode, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    state
        .service
        .uninstall(
            &caller.authorization.macro_user_id,
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/marketplace/listings/{id}/install/copy", tag = "marketplace", operation_id = "copy_marketplace_install",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    responses((status = 201, body = ListingCopy), (status = 400, body = ErrorResponse), (status = 401), (status = 404, body = ErrorResponse), (status = 500)))]
/// Make the caller's own copy of what they installed, recording the listing
/// and version it came from. An artifact becomes a new HTML document owned
/// by the caller, holding the version their install shows; the install
/// stays. A routine install is detached: the routine stays the caller's and
/// gets no more updates. Publishing the copy later says it is based on this
/// listing.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn copy_marketplace_install<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<(StatusCode, Json<ListingCopy>), MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let copy = state
        .service
        .copy(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(copy)))
}

#[utoipa::path(put, path = "/marketplace/listings/{id}/install/pin", tag = "marketplace", operation_id = "pin_marketplace_install",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    responses((status = 200, body = Listing), (status = 400, body = ErrorResponse), (status = 401), (status = 404, body = ErrorResponse), (status = 500)))]
/// Pin the caller's install to the version it has: a page keeps showing it
/// and a routine gets no automatic updates. Taking a version with
/// `PUT .../install` unpins it. `400` means a routine the caller edited or
/// made their own, which gets no updates to hold back.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn pin_marketplace_install<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<Json<Listing>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let listing = state
        .service
        .set_pinned(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            true,
        )
        .await?;
    Ok(Json(listing))
}

#[utoipa::path(delete, path = "/marketplace/listings/{id}/install/pin", tag = "marketplace", operation_id = "unpin_marketplace_install",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    responses((status = 200, body = Listing), (status = 401), (status = 404, body = ErrorResponse), (status = 500)))]
/// Unpin the caller's install: it moves to the newest minor version of its
/// major. A routine is updated to it unless its person edited it, which
/// detaches it as an automatic update would.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn unpin_marketplace_install<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<Json<Listing>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let listing = state
        .service
        .set_pinned(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            false,
        )
        .await?;
    Ok(Json(listing))
}

#[utoipa::path(get, path = "/marketplace/listings/{id}/install/build", tag = "marketplace", operation_id = "get_installed_listing_build",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    responses((status = 200, body = InstalledListingBuild), (status = 401), (status = 404), (status = 500), (status = 503)))]
/// Build the page the caller's install shows: the newest minor version of
/// the major they are on, or the version they pinned.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn get_installed_listing_build<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
) -> Result<Json<InstalledListingBuild>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let installed = state
        .service
        .installed_build(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(Json(installed.into()))
}

#[utoipa::path(post, path = "/marketplace/listings/{id}/install/code-executions", tag = "marketplace", operation_id = "run_installed_listing_program",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    request_body = RunDocumentProgram,
    responses((status = 200, body = UserExecutionReceipt), (status = 400), (status = 401), (status = 403), (status = 404), (status = 409), (status = 422), (status = 500), (status = 503)))]
/// Run one program of the page the caller's install shows, as the caller.
/// `409` means a newer minor version was published since the page was built.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn run_installed_listing_program<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    headers: HeaderMap,
    Json(body): Json<RunDocumentProgram>,
) -> Result<Json<UserExecutionReceipt>, MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    // Only a person in the app starts a run, as with a document's programs.
    if is_agent_run(caller.authorization.agent_id.is_some(), &headers) {
        return Err(MarketplaceError::Page(CodeModeError::Forbidden.into()));
    }

    let receipt = state
        .service
        .run_installed(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            RunProgram {
                source_hash: body.source_hash,
                program: body.program,
                arguments: body.arguments,
                timeout_ms: body.timeout_ms,
            },
            CancellationToken::new(),
        )
        .await?;
    Ok(Json(receipt))
}
