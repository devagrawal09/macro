//! Axum router for the marketplace.

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
use entity_access::{
    domain::{
        models::{AccessError, EntityAccessReceipt, EntityType, MemberTeamRole, OwnerAccessLevel},
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
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::domain::{
    models::{
        InstalledBuild, Listing, ListingId, ListingQuery, ListingText, ListingsShown,
        MarketplaceError, Version, VersionBump,
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
            post(publish_marketplace_listing_version::<S, Access, Auth>),
        )
        .route(
            "/marketplace/listings/{id}/install",
            put(install_marketplace_listing::<S, Access, Auth>)
                .delete(uninstall_marketplace_listing::<S, Access, Auth>),
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
    /// Only listings published from this document.
    pub document_id: Option<Uuid>,
}

/// Publish a page to the caller's team.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishListing {
    /// A stored HTML document the caller owns.
    pub document_id: Uuid,
    /// The listing's name.
    pub title: String,
    /// What it does, in a few lines.
    #[serde(default)]
    pub description: String,
}

/// Publish a listing's page as its next version.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishListingVersion {
    /// The document the listing was published from.
    pub document_id: Uuid,
    /// Whether installs get it at once (`minor`) or wait (`major`).
    pub bump: VersionBump,
}

/// The build of the page an install shows.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledListingBuild {
    /// The version built: the newest minor of the major the caller is on.
    pub version: Version,
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
            Self::Invalid(_) => StatusCode::BAD_REQUEST,
            Self::NoTeam | Self::AlreadyPublished | Self::Unchanged => StatusCode::CONFLICT,
            Self::DoesNotBuild => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Page(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match self {
            Self::Page(error) => return DocumentProgramApiError::from(error).into_response(),
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

/// The caller's owner receipt for a document they want to publish. A document
/// they can't see, or don't own, answers like one that doesn't exist.
async fn owned_document<S, Access: EntityAccessService, Auth>(
    state: &MarketplaceRouterState<S, Access, Auth>,
    user: &MacroUserIdStr<'static>,
    document_id: Uuid,
) -> Result<EntityAccessReceipt<OwnerAccessLevel>, MarketplaceError> {
    state
        .access
        .generate_entity_access_receipt::<OwnerAccessLevel>(
            user,
            None,
            &document_id.to_string(),
            EntityType::Document,
        )
        .await
        .map_err(|error| match error {
            AccessError::Unauthorized
            | AccessError::UnauthorizedWithMessage(_)
            | AccessError::NotFound(_)
            | AccessError::BadRequest(_) => MarketplaceError::NotFound,
            AccessError::Unavailable(_) => MarketplaceError::Page(DocumentProgramError::Busy),
            AccessError::Internal(report) => MarketplaceError::Internal(report),
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
        params.document_id,
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
/// Publish an HTML document the caller owns to their team's marketplace, as
/// version 1.0. `409` means the caller has no team or the document already
/// has a listing; `422` means the page doesn't build.
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
    let document = owned_document(&state, user, body.document_id).await?;
    let listing = state
        .service
        .publish(
            user,
            team.entity_access_receipt.as_ref(),
            document,
            ListingText {
                title: body.title,
                description: body.description,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(listing)))
}

#[utoipa::path(post, path = "/marketplace/listings/{id}/versions", tag = "marketplace", operation_id = "publish_marketplace_listing_version",
    params(("id" = Uuid, Path, description = "The listing")),
    request_body = PublishListingVersion,
    responses((status = 201, body = Listing), (status = 400, body = ErrorResponse), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 413), (status = 422, body = ErrorResponse), (status = 500), (status = 503)))]
/// Publish the listing's document as its next version. A minor version
/// reaches every install on the newest major at once; a major version waits
/// for each person to take it. `409` means the page hasn't changed.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn publish_marketplace_listing_version<S, Access, Auth>(
    State(state): State<MarketplaceRouterState<S, Access, Auth>>,
    Path(id): Path<Uuid>,
    caller: Caller<Auth>,
    team: Team<Access, Auth>,
    Json(body): Json<PublishListingVersion>,
) -> Result<(StatusCode, Json<Listing>), MarketplaceError>
where
    S: MarketplaceService,
    Access: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let user = &caller.authorization.macro_user_id;
    let document = owned_document(&state, user, body.document_id).await?;
    let listing = state
        .service
        .publish_version(
            user,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
            document,
            body.bump,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(listing)))
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
    responses((status = 200, body = Listing), (status = 401), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 500)))]
/// Put the caller on the listing's newest major version: installs it, or
/// takes a major version they were waiting on. Repeating it changes nothing.
#[tracing::instrument(err, skip_all, fields(%id))]
pub async fn install_marketplace_listing<S, Access, Auth>(
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
        .install(
            &caller.authorization.macro_user_id,
            team.entity_access_receipt.as_ref(),
            ListingId::from_uuid(id),
        )
        .await?;
    Ok(Json(listing))
}

#[utoipa::path(delete, path = "/marketplace/listings/{id}/install", tag = "marketplace", operation_id = "uninstall_marketplace_listing",
    params(("id" = Uuid, Path, description = "The listing")),
    responses((status = 204), (status = 401), (status = 404, body = ErrorResponse), (status = 500)))]
/// Remove the caller's install. Repeating it changes nothing.
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

#[utoipa::path(get, path = "/marketplace/listings/{id}/install/build", tag = "marketplace", operation_id = "get_installed_listing_build",
    params(("id" = Uuid, Path, description = "A listing the caller installed")),
    responses((status = 200, body = InstalledListingBuild), (status = 401), (status = 404), (status = 500), (status = 503)))]
/// Build the page the caller's install shows: the newest minor version of
/// the major they are on.
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
