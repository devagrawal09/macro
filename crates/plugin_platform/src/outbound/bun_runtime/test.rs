use std::path::PathBuf;

use super::{BunServerPluginRuntime, DEFAULT_BUN_BIN, DEFAULT_CLI_PATH, report_error};
use crate::domain::ports::{ServerPluginInvocation, ServerPluginOutcome, ServerPluginRuntime};

fn sample_invocation(bundle: &str) -> ServerPluginInvocation {
    ServerPluginInvocation {
        bundle_path: PathBuf::from(bundle),
        event_type: "task.created".into(),
        event: serde_json::json!({ "task_id": "t1" }),
        project_id: "p1".into(),
        installation_id: "i1".into(),
        capabilities: vec!["task:read".into()],
    }
}

#[test]
fn run_once_args_are_in_cli_order() {
    let runner = BunServerPluginRuntime::new(DEFAULT_BUN_BIN, PathBuf::from(DEFAULT_CLI_PATH));
    let args = runner.run_once_args(&sample_invocation("/tmp/bundle.js"));
    assert_eq!(
        args,
        vec![
            DEFAULT_CLI_PATH,
            "run-once",
            "/tmp/bundle.js",
            "--event-type",
            "task.created",
            "--event",
            r#"{"task_id":"t1"}"#,
            "--project-id",
            "p1",
            "--installation-id",
            "i1",
            "--capabilities",
            "task:read",
        ]
    );
}

#[test]
fn capabilities_flag_is_omitted_when_empty() {
    let runner = BunServerPluginRuntime::new(DEFAULT_BUN_BIN, PathBuf::from(DEFAULT_CLI_PATH));
    let mut invocation = sample_invocation("/tmp/bundle.js");
    invocation.capabilities.clear();
    let args = runner.run_once_args(&invocation);
    assert!(!args.contains(&"--capabilities".to_string()));
}

#[test]
fn cli_error_report_includes_exit_status_and_stderr() {
    let error = report_error(
        "plugin-runtime CLI",
        Some(std::process::ExitStatus::default()),
        b"MPC-R001 usage: run-once <bundle.js>",
    );
    assert!(error.to_string().contains("plugin-runtime CLI failed"));
    assert!(error.to_string().contains("MPC-R001 usage"));
}

/// Writes a tiny compiled-style JS fixture bundle and returns its path.
fn write_bundle(dir: &std::path::Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write fixture bundle");
    path
}

async fn real_runner() -> Option<BunServerPluginRuntime> {
    match tokio::process::Command::new(DEFAULT_BUN_BIN)
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
        Ok(runner) => Some(runner),
        Err(error) => {
            eprintln!("skipping: {error}");
            None
        }
    }
}

#[tokio::test]
async fn invokes_fixture_handler_through_real_cli() {
    let Some(runner) = real_runner().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("plugin-platform-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bundle = write_bundle(
        &dir,
        "ok.js",
        r#"
        export function handle(event, context) {
            if (!context.capabilities.includes("task:read")) throw new Error("missing capability");
            context.log.info("handled", { taskId: event.task_id });
        }
        "#,
    );

    let outcome = runner
        .invoke(sample_invocation(bundle.to_str().expect("utf8 path")))
        .await;
    match outcome {
        Ok(ServerPluginOutcome::Completed { duration_ms, .. }) => {
            assert!(duration_ms < 60_000);
        }
        Ok(other) => panic!("expected completed outcome, got {other:?}"),
        Err(error) => panic!("expected successful CLI run, got {error}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn surfaces_handler_failure_from_real_cli() {
    let Some(runner) = real_runner().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("plugin-platform-fail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bundle = write_bundle(
        &dir,
        "boom.js",
        "export function handle() { throw new Error('handler exploded'); }\n",
    );

    let outcome = runner
        .invoke(sample_invocation(bundle.to_str().expect("utf8 path")))
        .await;
    match outcome {
        Ok(ServerPluginOutcome::Failed { error, .. }) => {
            assert_eq!(error.code, "handler_error");
            assert!(error.message.contains("handler exploded"));
        }
        Ok(other) => panic!("expected failed outcome, got {other:?}"),
        Err(error) => panic!("expected structured failure, got {error}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}
