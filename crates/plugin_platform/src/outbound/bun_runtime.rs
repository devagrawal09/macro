//! Runs admitted server plugins by shelling out to the Bun plugin-runtime CLI.
//!
//! This is the local/dev POC runtime: one invocation, one `bun` process, one
//! structured result. The CLI (`services/plugin-runtime/src/cli.ts`) owns the
//! timeout, concurrency, and logging policy; this adapter only builds the
//! argument list, spawns the process, and parses its JSON report.

use std::path::PathBuf;
use std::process::ExitStatus;

use async_trait::async_trait;
use macro_env_var::maybe_env_vars;
use serde::Deserialize;
use tokio::process::Command;

use crate::domain::ports::{
    PortError, ServerPluginInvocation, ServerPluginOutcome, ServerPluginRuntime,
};

#[cfg(test)]
mod test;

maybe_env_vars! {
    /// Override of the `bun` binary. Defaults to plain `bun` resolved from PATH.
    struct PluginRuntimeBunBin;
    /// Override of the plugin-runtime CLI entrypoint path. Defaults to
    /// `services/plugin-runtime/src/cli.ts` relative to the current directory.
    struct PluginRuntimeCliPath;
}

/// Default `bun` binary name resolved from PATH.
pub const DEFAULT_BUN_BIN: &str = "bun";
/// Default CLI path relative to the repository root (the usual working directory).
pub const DEFAULT_CLI_PATH: &str = "services/plugin-runtime/src/cli.ts";

/// Bun-CLI-backed [`ServerPluginRuntime`] for local development.
#[derive(Debug, Clone)]
pub struct BunServerPluginRuntime {
    bun_binary: String,
    cli_path: PathBuf,
}

/// The `{ outcome, runs }` envelope printed by the run-once CLI.
#[derive(Debug, Deserialize)]
struct CliReport {
    outcome: ServerPluginOutcome,
}

impl BunServerPluginRuntime {
    /// Wrap an explicit `bun` binary and CLI path.
    pub fn new(bun_binary: impl Into<String>, cli_path: impl Into<PathBuf>) -> Self {
        Self {
            bun_binary: bun_binary.into(),
            cli_path: cli_path.into(),
        }
    }

    /// Resolve the runtime from the environment with dev defaults.
    ///
    /// Reads `PLUGIN_RUNTIME_BUN_BIN` (default `bun`) and
    /// `PLUGIN_RUNTIME_CLI_PATH` (default `services/plugin-runtime/src/cli.ts`
    /// relative to the current directory). Fails with a clear message when the
    /// configured CLI file does not exist.
    pub fn from_env() -> Result<Self, PortError> {
        let bun_binary = PluginRuntimeBunBin::new()
            .and_then(|value| value.value().map(str::to_string))
            .unwrap_or_else(|| DEFAULT_BUN_BIN.to_string());
        let cli_path = PluginRuntimeCliPath::new()
            .and_then(|value| value.value().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CLI_PATH));
        if !cli_path.is_file() {
            return Err(PortError(format!(
                "plugin-runtime CLI not found at {:?}; set PLUGIN_RUNTIME_CLI_PATH to \
                 services/plugin-runtime/src/cli.ts (or a built dist copy)",
                cli_path.display()
            )));
        }
        Ok(Self {
            bun_binary,
            cli_path,
        })
    }

    /// Pure argument list for one `run-once` invocation.
    ///
    /// The part that is easy to get wrong is argument order, not the spawn,
    /// so it is built and asserted without a process.
    fn run_once_args(&self, invocation: &ServerPluginInvocation) -> Vec<String> {
        let mut args = vec![
            self.cli_path.display().to_string(),
            "run-once".into(),
            invocation.bundle_path.display().to_string(),
            "--event-type".into(),
            invocation.event_type.clone(),
            "--event".into(),
            invocation.event.to_string(),
            "--project-id".into(),
            invocation.project_id.clone(),
            "--installation-id".into(),
            invocation.installation_id.clone(),
        ];
        if !invocation.capabilities.is_empty() {
            args.push("--capabilities".into());
            args.push(invocation.capabilities.join(","));
        }
        args
    }
}

fn report_error(context: &str, status: Option<ExitStatus>, stderr: &[u8]) -> PortError {
    let stderr = String::from_utf8_lossy(stderr);
    let trimmed = stderr.trim();
    let suffix = if trimmed.is_empty() {
        String::new()
    } else {
        format!(": {trimmed}")
    };
    match status {
        Some(status) => PortError(format!("{context} failed with {status}{suffix}")),
        None => PortError(format!("{context} failed{suffix}")),
    }
}

#[async_trait]
impl ServerPluginRuntime for BunServerPluginRuntime {
    async fn invoke(
        &self,
        invocation: ServerPluginInvocation,
    ) -> Result<ServerPluginOutcome, PortError> {
        let args = self.run_once_args(&invocation);
        let output = Command::new(&self.bun_binary)
            .args(&args)
            .output()
            .await
            .map_err(|error| {
                PortError(format!(
                    "failed to spawn bun binary {:?}: {error}",
                    self.bun_binary
                ))
            })?;

        // Exit code 2 means usage/config failure; no structured outcome exists.
        if output.status.code() == Some(2) {
            return Err(report_error(
                "plugin-runtime CLI",
                Some(output.status),
                &output.stderr,
            ));
        }
        let report: CliReport = serde_json::from_slice(&output.stdout).map_err(|error| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            PortError(format!(
                "could not parse plugin-runtime CLI report ({error}); exit={:?} stderr={}",
                output.status.code(),
                stderr.trim()
            ))
        })?;
        Ok(report.outcome)
    }
}
