//! Authenticated SSE adapter for receipt-filtered live task events.

#[cfg(test)]
mod test;

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    extract::{FromRef, Path, State},
    http::Request,
    middleware::{self, Next},
    response::sse::{Event, KeepAlive, Sse},
};
use documents::domain::task_events::{TaskEvent, TaskEventSubscriptionService};
use entity_access::{
    domain::{models::ViewAccessLevel, ports::EntityAccessService},
    inbound::axum_extractors::ProjectAccessLevelExtractor,
};
use futures::{Stream, StreamExt};
use macro_authorization::{
    AnyPrincipal, MacroAuthorizationExtractor, MacroAuthorizationService, MacroAuthorizationState,
};

use super::Params;
use crate::domain::{models::ProjectError, ports::ProjectService};
use model::project::BasicProject;

/// Narrow project-loading capability required by the task-events adapter.
pub trait TaskEventsProjectLoader: Send + Sync + 'static {
    /// Load the standard project context used by the access extractor.
    fn load_task_events_project(
        &self,
        project_id: &str,
    ) -> impl std::future::Future<Output = Result<BasicProject, ProjectError>> + Send;
}

impl<T> TaskEventsProjectLoader for T
where
    T: ProjectService,
{
    async fn load_task_events_project(
        &self,
        project_id: &str,
    ) -> Result<BasicProject, ProjectError> {
        self.internal_get_basic_project(project_id).await
    }
}

/// Dedicated dependencies for the task-events endpoint.
pub struct TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth> {
    /// Project service used only to load the standard project context.
    pub project_service: Arc<ProjectSvc>,
    /// Receipt-filtering task-event domain service.
    pub subscription_service: Arc<SubscriptionSvc>,
    /// Entity-access service used by the standard access extractor.
    pub access_service: Arc<Svc>,
    /// Request authorization state.
    pub authorization_state: MacroAuthorizationState<Auth>,
}

impl<ProjectSvc, SubscriptionSvc, Svc, Auth> Clone
    for TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>
{
    fn clone(&self) -> Self {
        Self {
            project_service: self.project_service.clone(),
            subscription_service: self.subscription_service.clone(),
            access_service: self.access_service.clone(),
            authorization_state: self.authorization_state.clone(),
        }
    }
}

impl<ProjectSvc, SubscriptionSvc, Svc, Auth>
    FromRef<TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>> for Arc<Svc>
{
    fn from_ref(state: &TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>) -> Self {
        state.access_service.clone()
    }
}

impl<ProjectSvc, SubscriptionSvc, Svc, Auth>
    FromRef<TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>>
    for MacroAuthorizationState<Auth>
{
    fn from_ref(state: &TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>) -> Self {
        state.authorization_state.clone()
    }
}

/// Stream live task events for one authorized project.
#[utoipa::path(
    get,
    path = "/projects/{id}/task_events",
    params(("id" = String, Path, description = "ID of the project")),
    responses(
        (status = 200, content_type = "text/event-stream", body = TaskEvent),
        (status = 401, body = model::response::GenericErrorResponse),
        (status = 500, body = model::response::GenericErrorResponse),
    )
)]
#[tracing::instrument(skip(state, _authorization, access))]
pub async fn task_events_handler<ProjectSvc, SubscriptionSvc, Svc, Auth>(
    State(state): State<TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>>,
    _authorization: MacroAuthorizationExtractor<Auth, AnyPrincipal>,
    access: ProjectAccessLevelExtractor<ViewAccessLevel, Svc, Auth>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
    ProjectSvc: TaskEventsProjectLoader,
    SubscriptionSvc: TaskEventSubscriptionService,
    Svc: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let stream = state
        .subscription_service
        .subscribe(access.entity_access_receipt)
        .map(|event| {
            let data = serde_json::to_string(&event)
                .expect("serializing TaskEvent with string fields cannot fail");
            Ok(Event::default().data(data))
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// Build the dedicated task-events router.
pub fn task_events_router<ProjectSvc, SubscriptionSvc, Svc, Auth, S>(
    state: TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>,
) -> Router<S>
where
    ProjectSvc: TaskEventsProjectLoader,
    SubscriptionSvc: TaskEventSubscriptionService + 'static,
    Svc: EntityAccessService,
    Auth: MacroAuthorizationService,
    S: Send + Sync + 'static,
{
    Router::new()
        .route(
            "/{id}/task_events",
            axum::routing::get(task_events_handler::<ProjectSvc, SubscriptionSvc, Svc, Auth>),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            load_project_context::<ProjectSvc, SubscriptionSvc, Svc, Auth>,
        ))
        .with_state(state)
}

#[tracing::instrument(skip(state, request, next), err)]
async fn load_project_context<ProjectSvc, SubscriptionSvc, Svc, Auth>(
    State(state): State<TaskEventsRouterState<ProjectSvc, SubscriptionSvc, Svc, Auth>>,
    Path(Params { id }): Path<Params>,
    mut request: Request<Body>,
    next: Next,
) -> Result<axum::response::Response, ProjectError>
where
    ProjectSvc: TaskEventsProjectLoader,
    SubscriptionSvc: TaskEventSubscriptionService,
    Svc: EntityAccessService,
    Auth: MacroAuthorizationService,
{
    let project = state.project_service.load_task_events_project(&id).await?;
    request.extensions_mut().insert(project);
    Ok(next.run(request).await)
}
