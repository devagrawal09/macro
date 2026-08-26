use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use plugin_platform::domain::ports::PluginInvocationError;
use serde_json::json;
use tower::ServiceExt;

/// Valid compiler manifest in the canonical `@macro/plugin-cli` shape.
fn valid_manifest() -> Value {
    json!({
        "apiVersion": "1",
        "plugin": {
            "id": "com.macro.task-inbox",
            "name": "Task Inbox",
            "version": "1.0.0"
        },
        "entrypoints": {
            "inbox": {
                "target": "client",
                "file": "client/inbox/index.js",
                "integrity": "sha256-client"
            }
        },
    })
}

fn completed_outcome() -> ServerPluginOutcome {
    ServerPluginOutcome::Completed {
        run_id: "run-1".into(),
        duration_ms: 12,
    }
}

/// Runtime fake that records expectations and reports one fixed outcome.
struct FakeRuntime {
    outcome: ServerPluginOutcome,
}

#[async_trait::async_trait]
impl ServerPluginRuntime for FakeRuntime {
    async fn invoke(
        &self,
        invocation: ServerPluginInvocation,
    ) -> Result<ServerPluginOutcome, PortError> {
        assert_eq!(invocation.event_type, "task.created");
        assert_eq!(invocation.project_id, "p1");
        Ok(self.outcome.clone())
    }
}

fn test_router(outcome: ServerPluginOutcome) -> axum::Router {
    settings_router(outcome, None)
}

fn settings_router(
    outcome: ServerPluginOutcome,
    settings: Option<Arc<crate::settings::PluginSettingsStore>>,
) -> axum::Router {
    plugin_router(PluginHttpState {
        runtime: Arc::new(FakeRuntime { outcome }),
        settings,
    })
}

async fn post_json(
    router: axum::Router,
    path: &str,
    body: Value,
) -> (axum::http::StatusCode, Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("build request"),
        )
        .await
        .expect("in-process response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let parsed = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };
    (status, parsed)
}

async fn get_json(router: axum::Router, path: &str) -> (axum::http::StatusCode, Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("in-process response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let parsed = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };
    (status, parsed)
}

#[tokio::test]
async fn admit_returns_the_admitted_manifest() {
    let (status, body) = post_json(
        test_router(completed_outcome()),
        "/plugins/admit",
        valid_manifest(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pluginId"], "com.macro.task-inbox");
    assert_eq!(body["version"], "1.0.0");
    assert!(body["entrypoints"]["inbox"].is_object());
}

#[tokio::test]
async fn admit_rejects_an_invalid_manifest_with_a_structured_error() {
    let mut invalid = valid_manifest();
    invalid.as_object_mut().unwrap().remove("entrypoints");
    let (status, body) =
        post_json(test_router(completed_outcome()), "/plugins/admit", invalid).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_manifest");
    assert!(body["error"]["message"].as_str().is_some());
}

#[tokio::test]
async fn invoke_returns_the_structured_outcome() {
    let request = json!({
        "bundle_path": "/tmp/ok.js",
        "event_type": "task.created",
        "event": { "task_id": "t1" },
        "project_id": "p1",
        "installation_id": "i1",
        "capabilities": ["task:read"]
    });
    let (status, body) =
        post_json(test_router(completed_outcome()), "/plugins/invoke", request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "completed");
    // Outcome fields serialize camelCase to mirror the executor's report.
    assert_eq!(body["runId"], "run-1");
}

#[tokio::test]
async fn invoke_maps_a_failed_handler_to_its_structured_outcome() {
    let outcome = ServerPluginOutcome::Failed {
        run_id: "run-2".into(),
        duration_ms: 3,
        error: PluginInvocationError {
            code: "handler_error".into(),
            message: "boom".into(),
        },
    };
    let request = json!({
        "bundle_path": "/tmp/bad.js",
        "event_type": "task.created",
        "event": {},
        "project_id": "p1",
        "installation_id": "i1",
        "capabilities": []
    });
    let (status, body) = post_json(test_router(outcome), "/plugins/invoke", request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "failed");
    assert_eq!(body["error"]["code"], "handler_error");
}

/// Real Bun runtime when available; otherwise the caller skips the test.
async fn real_router() -> Option<axum::Router> {
    use plugin_platform::outbound::bun_runtime::BunServerPluginRuntime;

    match tokio::process::Command::new(plugin_platform::outbound::bun_runtime::DEFAULT_BUN_BIN)
        .arg("--version")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {}
        _ => {
            eprintln!("skipping: bun is not available on PATH");
            return None;
        }
    }
    match BunServerPluginRuntime::from_env() {
        Ok(runtime) => Some(plugin_router(PluginHttpState {
            runtime: Arc::new(runtime),
            settings: None,
        })),
        Err(error) => {
            eprintln!("skipping: {error}");
            None
        }
    }
}

#[tokio::test]
async fn invoke_runs_a_real_bundle_through_bun_when_available() {
    let Some(router) = real_router().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("plugin-http-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bundle: PathBuf = dir.join("ok.js");
    std::fs::write(
        &bundle,
        r#"
        export function handle(event, context) {
            context.log.info("handled", { taskId: event.task_id });
        }
        "#,
    )
    .expect("write fixture bundle");

    let request = json!({
        "bundle_path": bundle.to_str().expect("utf8 path"),
        "event_type": "task.created",
        "event": { "task_id": "t1" },
        "project_id": "p1",
        "installation_id": "i1",
        "capabilities": ["task:read"]
    });
    let (status, body) = post_json(router, "/plugins/invoke", request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "completed", "{body}");

    std::fs::remove_dir_all(&dir).ok();
}

// --- /plugins/settings endpoints ---

fn seeded_settings() -> Arc<crate::settings::PluginSettingsStore> {
    Arc::new(crate::settings::PluginSettingsStore::seeded())
}

#[tokio::test]
async fn settings_returns_the_seeded_snapshot() {
    let (status, body) = get_json(
        settings_router(completed_outcome(), Some(seeded_settings())),
        "/plugins/settings",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["installation"]["pluginId"], "dev.local.task-tools");
    assert_eq!(body["installation"]["enabled"], true);
    assert_eq!(body["installation"]["verified"], false);
    assert_eq!(body["installation"]["clientCapabilities"][0], "tasks.read");
    assert_eq!(body["installation"]["project"]["id"], "proj-demo-1");
    let handlers = body["installation"]["handlers"]
        .as_array()
        .expect("handlers");
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0]["event"], "task.created");
    assert_eq!(handlers[0]["paused"], false);
    // Runs are newest first.
    let runs = body["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0]["id"], "run-3");
    assert!(
        runs[0]["startedAt"].as_u64().expect("startedAt")
            > runs[2]["startedAt"].as_u64().expect("startedAt")
    );
    // errorSummary is omitted when absent.
    assert!(runs[0].get("errorSummary").is_none());
    assert_eq!(runs[1]["errorSummary"], "Task not found (404)");
}

#[tokio::test]
async fn settings_endpoints_report_when_state_is_absent() {
    let (status, body) = get_json(test_router(completed_outcome()), "/plugins/settings").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "settings_unavailable");
}

#[tokio::test]
async fn enabled_toggle_persists_on_shared_state_and_round_trips() {
    let settings = seeded_settings();
    let router = settings_router(completed_outcome(), Some(settings.clone()));

    let (status, body) = post_json(
        router,
        "/plugins/settings/enabled",
        json!({ "enabled": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["installation"]["enabled"], false);

    // A later GET over the same shared state still sees the toggle.
    let snapshot = settings.snapshot();
    assert!(!snapshot.installation.enabled);
}

#[tokio::test]
async fn handler_pause_updates_only_the_matching_handler() {
    let settings = seeded_settings();
    let path = "/plugins/settings/handlers/normalize-created-task/paused";
    let (status, body) = post_json(
        settings_router(completed_outcome(), Some(settings.clone())),
        path,
        json!({ "paused": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["installation"]["handlers"][0]["paused"], true);

    let snapshot = settings.snapshot();
    assert!(snapshot.installation.handlers[0].paused);
}

#[tokio::test]
async fn unknown_handler_pause_is_a_structured_404() {
    let (status, body) = post_json(
        settings_router(completed_outcome(), Some(seeded_settings())),
        "/plugins/settings/handlers/does-not-exist/paused",
        json!({ "paused": true }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "handler_not_found");
    assert!(body["error"]["message"].as_str().is_some());
}

#[tokio::test]
async fn invoke_appends_a_completed_run_to_the_history() {
    let settings = seeded_settings();
    let router = settings_router(completed_outcome(), Some(settings.clone()));
    let request = json!({
        "bundle_path": "/tmp/ok.js",
        "event_type": "task.created",
        "event": { "task_id": "t1" },
        "project_id": "p1",
        "installation_id": "i1",
        "capabilities": ["task:read"]
    });
    let (status, _) = post_json(router, "/plugins/invoke", request).await;
    assert_eq!(status, StatusCode::OK);

    let snapshot = settings.snapshot();
    assert_eq!(snapshot.runs.len(), 4);
    // Runs stay sorted newest first even with the future-dated seeds.
    let started_ats = snapshot
        .runs
        .iter()
        .map(|run| run.started_at)
        .collect::<Vec<_>>();
    let mut sorted = started_ats.clone();
    sorted.sort_unstable();
    sorted.reverse();
    assert_eq!(started_ats, sorted);
    let recorded = snapshot
        .runs
        .iter()
        .find(|run| run.id == "run-1" && run.duration_ms == 12)
        .expect("recorded run");
    assert_eq!(recorded.outcome, "completed");
    // The event type resolved to the configured handler id.
    assert_eq!(recorded.handler_id, "normalize-created-task");
    assert!(recorded.error_summary.is_none());
}

#[tokio::test]
async fn invoke_appends_a_failed_run_with_an_error_summary() {
    let outcome = ServerPluginOutcome::Failed {
        run_id: "run-9".into(),
        duration_ms: 5,
        error: PluginInvocationError {
            code: "handler_error".into(),
            message: "boom".into(),
        },
    };
    let settings = seeded_settings();
    let router = settings_router(outcome, Some(settings.clone()));
    let request = json!({
        "bundle_path": "/tmp/bad.js",
        "event_type": "task.created",
        "event": {},
        "project_id": "p1",
        "installation_id": "i1",
        "capabilities": []
    });
    let (status, _) = post_json(router, "/plugins/invoke", request).await;
    assert_eq!(status, StatusCode::OK);

    let snapshot = settings.snapshot();
    assert_eq!(snapshot.runs.len(), 4);
    let recorded = snapshot
        .runs
        .iter()
        .find(|run| run.id == "run-9")
        .expect("recorded run");
    assert_eq!(recorded.outcome, "failed");
    assert_eq!(recorded.error_summary.as_deref(), Some("boom"));
}

#[tokio::test]
async fn cors_headers_are_emitted_for_browser_origins() {
    let router = test_router(completed_outcome());

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .header("Origin", "http://localhost:3000")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );

    // Browser preflight for JSON POSTs must short-circuit with CORS headers.
    let preflight = router
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/plugins/settings/enabled")
                .header("Origin", "http://localhost:3000")
                .header("Access-Control-Request-Method", "POST")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(preflight.status(), 200);
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
}
