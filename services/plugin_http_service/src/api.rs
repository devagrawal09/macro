//! Thin inbound HTTP adapter over the Plugin Platform core.
//!
//! This is a dev-only POC surface: it is unauthenticated and exposes exactly
//! two use cases, manifest admission and one-shot server-plugin invocation.
//! All business rules stay in `plugin_platform::domain`; handlers only parse
//! transport data, call the domain/runtime, and map results to HTTP.

#[cfg(test)]
mod test;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use plugin_platform::domain::errors::PluginPlatformError;
use plugin_platform::domain::models::PluginManifest;
use plugin_platform::domain::ports::PortError;
use plugin_platform::domain::ports::ServerPluginInvocation;
use plugin_platform::domain::ports::ServerPluginOutcome;
use plugin_platform::domain::ports::ServerPluginRuntime;
use rootcause::Report;
use serde::Deserialize;
use serde_json::Value;

use crate::settings::PluginSettingsStore;
use crate::settings::SettingsSnapshot;

/// Shared inbound state for the plugin HTTP surface.
pub struct PluginHttpState<R> {
    /// Runtime that executes admitted server-plugin invocations.
    pub runtime: Arc<R>,
    /// In-memory settings state backing the `/plugins/settings*` endpoints.
    /// `None` keeps those endpoints unavailable (e.g. in focused tests).
    pub settings: Option<Arc<PluginSettingsStore>>,
}

impl<R> Clone for PluginHttpState<R> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            settings: self.settings.clone(),
        }
    }
}

/// Build the dev-only plugin HTTP router.
pub fn plugin_router<R: ServerPluginRuntime + 'static>(state: PluginHttpState<R>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/plugins/admit", post(admit_handler))
        .route("/plugins/invoke", post(invoke_handler::<R>))
        .route("/plugins/settings", get(settings_handler::<R>))
        .route("/plugins/settings/enabled", post(set_enabled_handler::<R>))
        .route(
            "/plugins/settings/handlers/{handler_id}/paused",
            post(set_handler_paused_handler::<R>),
        )
        .with_state(state)
}

async fn health_handler() -> &'static str {
    "healthy"
}

/// Request body for POST /plugins/invoke.
#[derive(Debug, Deserialize)]
struct InvokeRequest {
    /// Path to the compiled server entry bundle.
    bundle_path: std::path::PathBuf,
    /// Declared event name being delivered.
    event_type: String,
    /// Event payload passed verbatim to the handler.
    event: Value,
    /// Project scope of the installation that owns this invocation.
    project_id: String,
    /// Installation whose release is running.
    installation_id: String,
    /// Capabilities granted to this server entrypoint for this invocation.
    capabilities: Vec<String>,
}

/// Structured JSON error returned by every failing endpoint.
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn invalid_manifest(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_manifest",
            message,
        }
    }

    fn settings_unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "settings_unavailable",
            message: "settings state is not configured".into(),
        }
    }

    fn handler_not_found(handler_id: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "handler_not_found",
            message: format!("no handler with id `{handler_id}`"),
        }
    }

    fn dependency(error: PortError) -> Self {
        let report: Report = Report::from(error);
        tracing::error!(error=?report, "plugin platform dependency failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "dependency_failed",
            message: format!("{report}"),
        }
    }
}

impl From<PluginPlatformError> for ApiError {
    fn from(error: PluginPlatformError) -> Self {
        match &error {
            PluginPlatformError::InvalidArtifact(_) => Self::invalid_manifest(error.to_string()),
            _ => Self::dependency(PortError(error.to_string())),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": { "code": self.code, "message": self.message }
        });
        (self.status, Json(body)).into_response()
    }
}

/// POST /plugins/admit handler.
///
/// Admits a compiler manifest through the same domain path as
/// [`PluginManifest::from_value`] and returns the admitted manifest.
#[tracing::instrument(skip_all, err)]
async fn admit_handler(Json(manifest): Json<Value>) -> Result<Json<PluginManifest>, ApiError> {
    let admitted = PluginManifest::from_value(manifest).map_err(ApiError::from)?;
    Ok(Json(admitted))
}

/// POST /plugins/invoke handler.
///
/// Runs one admitted server-plugin invocation through the configured
/// [`ServerPluginRuntime`] and returns its structured terminal outcome.
#[tracing::instrument(skip_all, err)]
async fn invoke_handler<R: ServerPluginRuntime>(
    State(state): State<PluginHttpState<R>>,
    Json(request): Json<InvokeRequest>,
) -> Result<Json<ServerPluginOutcome>, ApiError> {
    let invocation = ServerPluginInvocation {
        bundle_path: request.bundle_path,
        event_type: request.event_type.clone(),
        event: request.event,
        project_id: request.project_id,
        installation_id: request.installation_id,
        capabilities: request.capabilities,
    };
    let outcome = state
        .runtime
        .invoke(invocation)
        .await
        .map_err(ApiError::dependency)?;
    // Started invocations land in the dev run history when settings state is
    // wired in; skipped events never start a run and are ignored there.
    if let Some(settings) = &state.settings {
        settings.record_run(&outcome, &request.event_type);
    }
    Ok(Json(outcome))
}

/// GET /plugins/settings handler.
///
/// Returns the current installation snapshot plus newest-first run history.
#[tracing::instrument(skip_all, err)]
async fn settings_handler<R>(
    State(state): State<PluginHttpState<R>>,
) -> Result<Json<SettingsSnapshot>, ApiError> {
    let settings = state
        .settings
        .as_ref()
        .ok_or_else(ApiError::settings_unavailable)?;
    Ok(Json(settings.snapshot()))
}

/// Body of POST /plugins/settings/enabled.
#[derive(Debug, Deserialize)]
struct SetEnabledRequest {
    /// New value for the installation's master enable switch.
    enabled: bool,
}

/// POST /plugins/settings/enabled handler.
///
/// Toggles the master enable switch and returns the updated snapshot.
#[tracing::instrument(skip_all, err)]
async fn set_enabled_handler<R>(
    State(state): State<PluginHttpState<R>>,
    Json(request): Json<SetEnabledRequest>,
) -> Result<Json<SettingsSnapshot>, ApiError> {
    let settings = state
        .settings
        .as_ref()
        .ok_or_else(ApiError::settings_unavailable)?;
    settings.set_enabled(request.enabled);
    Ok(Json(settings.snapshot()))
}

/// Body of POST /plugins/settings/handlers/{handlerId}/paused.
#[derive(Debug, Deserialize)]
struct SetHandlerPausedRequest {
    /// Whether deliveries to the handler are paused.
    paused: bool,
}

/// POST /plugins/settings/handlers/{handlerId}/paused handler.
///
/// Pauses or resumes one handler and returns the updated snapshot.
#[tracing::instrument(skip_all, err)]
async fn set_handler_paused_handler<R>(
    State(state): State<PluginHttpState<R>>,
    Path(handler_id): Path<String>,
    Json(request): Json<SetHandlerPausedRequest>,
) -> Result<Json<SettingsSnapshot>, ApiError> {
    let settings = state
        .settings
        .as_ref()
        .ok_or_else(ApiError::settings_unavailable)?;
    if !settings.set_handler_paused(&handler_id, request.paused) {
        return Err(ApiError::handler_not_found(&handler_id));
    }
    Ok(Json(settings.snapshot()))
}
