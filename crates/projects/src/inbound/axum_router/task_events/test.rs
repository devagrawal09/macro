use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use documents::domain::task_events::{
    TaskEvent, TaskEventStream, TaskEventSubscriptionService, TaskEventType,
};
use entity_access::domain::{
    models::{
        AccessError, AccessLevel, BotAccessScope, BotId, CallChannelInfo, EntityAccessReceipt,
        EntityPermission, EntityType, RequiredPermission, UserTeamInfo, ViewAccessLevel,
    },
    ports::EntityAccessService,
};
use http_body_util::BodyExt;
use macro_authorization::{
    InternalIdentityClaims, MacroAuthorizationError, MacroAuthorizationService,
    MacroAuthorizationState,
};
use macro_user_id::{lowercased::Lowercase, user_id::MacroUserId, user_id::MacroUserIdStr};
use model::project::BasicProject;
use model_user::UserContext;
use rootcause::Report;
use tower::ServiceExt;
use uuid::Uuid;

use super::*;

const TOKEN: &str = "valid-token";
const USER_ID: &str = "macro|task-events@example.com";
const PROJECT_ID: &str = "project-1";

#[derive(Clone)]
struct FakeProjectLoader;

impl TaskEventsProjectLoader for FakeProjectLoader {
    async fn load_task_events_project(
        &self,
        project_id: &str,
    ) -> Result<BasicProject, ProjectError> {
        Ok(BasicProject {
            id: project_id.to_string(),
            user_id: MacroUserIdStr::try_from(USER_ID.to_string()).unwrap(),
            parent_id: None,
            name: "Project".to_string(),
            deleted_at: None,
        })
    }
}

#[derive(Clone, Default)]
struct RecordingSubscription {
    receipt_projects: Arc<Mutex<Vec<String>>>,
}

impl TaskEventSubscriptionService for RecordingSubscription {
    fn subscribe(&self, receipt: EntityAccessReceipt<ViewAccessLevel>) -> TaskEventStream {
        let project_id = receipt.entity().entity_id.clone();
        self.receipt_projects
            .lock()
            .unwrap()
            .push(project_id.clone());
        Box::pin(futures::stream::iter([TaskEvent {
            event_id: "event".into(),
            event_type: TaskEventType::TaskCreated,
            document_id: "document".into(),
            project_id,
        }]))
    }
}

#[derive(Clone)]
struct FakeEntityAccessService;

impl EntityAccessService for FakeEntityAccessService {
    async fn generate_entity_access_receipt<T: RequiredPermission>(
        &self,
        _user_id: &MacroUserId<Lowercase<'_>>,
        _user_org_id: Option<i64>,
        _entity_id: &str,
        _entity_type: EntityType,
    ) -> Result<EntityAccessReceipt<T>, AccessError> {
        Err(AccessError::Unauthorized)
    }

    async fn generate_bot_entity_access_receipt<T: RequiredPermission>(
        &self,
        _bot_id: BotId,
        _scope: BotAccessScope,
        _entity_id: &str,
        _entity_type: EntityType,
    ) -> Result<EntityAccessReceipt<T>, AccessError> {
        panic!("unexpected bot receipt generation")
    }

    async fn get_access_level(
        &self,
        _user_id: Option<&MacroUserId<Lowercase<'_>>>,
        _entity_id: &str,
        _entity_type: EntityType,
    ) -> Result<Option<AccessLevel>, AccessError> {
        panic!("owner access should be derived from loaded project context")
    }

    async fn check_access(
        &self,
        _user_id: Option<&MacroUserId<Lowercase<'_>>>,
        _entity_id: &str,
        _entity_type: EntityType,
        _required_level: AccessLevel,
    ) -> Result<AccessLevel, AccessError> {
        panic!("unexpected access check")
    }

    async fn check_public_access(
        &self,
        _entity_id: &str,
        _entity_type: EntityType,
        _required_level: AccessLevel,
    ) -> Result<AccessLevel, AccessError> {
        panic!("unexpected public access check")
    }

    async fn get_entity_permission(
        &self,
        _user_id: Option<&MacroUserId<Lowercase<'_>>>,
        _entity_id: &str,
        _entity_type: EntityType,
        _user_org_id: Option<i64>,
    ) -> Result<EntityPermission, AccessError> {
        panic!("unexpected permission lookup")
    }

    async fn get_crm_entity_permission_with_team(
        &self,
        _user_id: Option<&MacroUserId<Lowercase<'_>>>,
        _entity_id: &str,
        _entity_type: EntityType,
    ) -> Result<
        (
            EntityPermission,
            Uuid,
            entity_access::domain::models::TeamRole,
        ),
        AccessError,
    > {
        panic!("unexpected CRM permission lookup")
    }

    async fn get_users_by_entity(
        &self,
        _entity_id: &str,
        _entity_type: EntityType,
    ) -> Result<Vec<MacroUserIdStr<'static>>, AccessError> {
        panic!("unexpected user lookup")
    }

    async fn get_call_channel(
        &self,
        _call_id: &Uuid,
    ) -> Result<Option<CallChannelInfo>, AccessError> {
        panic!("unexpected call lookup")
    }

    async fn get_call_channel_by_channel_id(
        &self,
        _channel_id: &Uuid,
    ) -> Result<Option<CallChannelInfo>, AccessError> {
        panic!("unexpected channel lookup")
    }

    async fn get_user_team(
        &self,
        _user_id: &MacroUserId<Lowercase<'_>>,
    ) -> Result<Option<UserTeamInfo>, AccessError> {
        panic!("unexpected team lookup")
    }
}

#[derive(Clone, Default)]
struct FakeAuthorizationService;

impl MacroAuthorizationService for FakeAuthorizationService {
    async fn authorize(&self, token: &str) -> Result<UserContext, Report<MacroAuthorizationError>> {
        if token != TOKEN {
            return Err(Report::new(MacroAuthorizationError::InvalidCredentials));
        }
        Ok(UserContext {
            user_id: USER_ID.to_string(),
            fusion_user_id: "fusion-user".to_string(),
            permissions: None,
            organization_id: None,
        })
    }

    async fn authorize_internal(
        &self,
        _provided_key: &str,
        _claims: InternalIdentityClaims,
    ) -> Result<Option<UserContext>, Report<MacroAuthorizationError>> {
        Err(Report::new(MacroAuthorizationError::InvalidCredentials))
    }
}

fn router(subscription: RecordingSubscription) -> Router {
    task_events_router::<
        FakeProjectLoader,
        RecordingSubscription,
        FakeEntityAccessService,
        FakeAuthorizationService,
        (),
    >(TaskEventsRouterState {
        project_service: Arc::new(FakeProjectLoader),
        subscription_service: Arc::new(subscription),
        access_service: Arc::new(FakeEntityAccessService),
        authorization_state: MacroAuthorizationState::new(Arc::new(FakeAuthorizationService)),
    })
}

#[tokio::test]
async fn task_events_requires_authentication() {
    let response = router(RecordingSubscription::default())
        .oneshot(
            Request::builder()
                .uri(format!("/{PROJECT_ID}/task_events"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn typed_project_receipt_reaches_domain_and_stream_is_exact_and_closes() {
    let subscription = RecordingSubscription::default();
    let receipts = subscription.receipt_projects.clone();
    let response = router(subscription)
        .oneshot(
            Request::builder()
                .uri(format!("/{PROJECT_ID}/task_events"))
                .header("authorization", format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(receipts.lock().unwrap().as_slice(), &[PROJECT_ID]);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = std::str::from_utf8(&body).unwrap();
    assert_eq!(
        body,
        "data: {\"event_id\":\"event\",\"event_type\":\"task.created\",\"document_id\":\"document\",\"project_id\":\"project-1\"}\n\n"
    );
    assert!(!body.lines().any(|line| line.starts_with("id:")));
}
