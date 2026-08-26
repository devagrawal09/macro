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
        "customEvents": [{ "name": "task.approved", "direction": "both" }],
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
    plugin_router(PluginHttpState {
        runtime: Arc::new(FakeRuntime { outcome }),
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
    assert_eq!(body["events"]["task.approved"]["direction"], "both");
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
    assert_eq!(body["run_id"], "run-1");
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
