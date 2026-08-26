//! In-memory settings state for the dev-only plugin HTTP surface.
//!
//! Holds one seeded installation (matching the frontend fixture's mock
//! shapes), its per-handler pause flags, and a started-run history. There is
//! deliberately no database: this is POC state for local development, and it
//! disappears with the process.

use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use plugin_platform::domain::ports::ServerPluginOutcome;
use serde::Serialize;

/// A project-scoped installation of one plugin release.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDto {
    /// Stable plugin identifier, e.g. `dev.local.task-tools`.
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Installed release version.
    pub version: String,
    /// Local/dev sideloads are unverified; production installs would be true.
    pub verified: bool,
    /// Master enable switch for the installation.
    pub enabled: bool,
    /// Capabilities granted to client contributions (browser side).
    pub client_capabilities: Vec<String>,
    /// Capabilities granted to server handlers (server side).
    pub server_capabilities: Vec<String>,
    /// Project scope of the installation, if any.
    pub project: Option<ProjectDto>,
    /// Server handlers declared by the installed release.
    pub handlers: Vec<HandlerDto>,
}

/// Project scope of an installation.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectDto {
    /// Project identifier.
    pub id: String,
    /// Human-readable project name.
    pub name: String,
}

/// One server handler declared by the installed release.
#[derive(Debug, Clone, Serialize)]
pub struct HandlerDto {
    /// Handler identifier within the release.
    pub id: String,
    /// Human-readable handler label.
    pub label: String,
    /// Event type this handler reacts to, e.g. `task.created`.
    pub event: String,
    /// Whether deliveries to this handler are currently paused.
    pub paused: bool,
}

/// One admitted/started handler run. Events that never start a run are
/// discarded by contract and never appear here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDto {
    /// Executor-assigned run identifier.
    pub id: String,
    /// Unix epoch milliseconds when the run started.
    pub started_at: u64,
    /// Handler that ran.
    pub handler_id: String,
    /// Terminal outcome of the run.
    pub outcome: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Bounded failure detail for failed/timeout runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
}

/// Full settings snapshot served by GET /plugins/settings.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsSnapshot {
    /// The single dev installation tracked by this service.
    pub installation: InstallationDto,
    /// Started runs only, newest first.
    pub runs: Vec<RunDto>,
}

#[derive(Debug)]
struct SettingsData {
    installation: InstallationDto,
    runs: Vec<RunDto>,
}

/// Shared, mutex-guarded settings state handed to the inbound handlers.
pub struct PluginSettingsStore {
    inner: Mutex<SettingsData>,
}

impl PluginSettingsStore {
    /// Build a store seeded with data matching the frontend fixture's mock
    /// shapes (the Task Tools trial plugin and three historical runs).
    pub fn seeded() -> Self {
        let installation = InstallationDto {
            plugin_id: "dev.local.task-tools".into(),
            name: "Task Tools".into(),
            version: "0.1.0".into(),
            verified: false,
            enabled: true,
            project: Some(ProjectDto {
                id: "proj-demo-1".into(),
                name: "Demo Project".into(),
            }),
            client_capabilities: vec![
                "tasks.read".into(),
                "tasks.create".into(),
                "task-events.read".into(),
            ],
            server_capabilities: vec!["tasks.read".into(), "tasks.rename".into()],
            handlers: vec![HandlerDto {
                id: "normalize-created-task".into(),
                label: "Normalize created task".into(),
                event: "task.created".into(),
                paused: false,
            }],
        };
        let runs = vec![
            RunDto {
                id: "run-3".into(),
                started_at: 1_787_735_643_000,
                handler_id: "normalize-created-task".into(),
                outcome: "completed".into(),
                duration_ms: 812,
                error_summary: None,
            },
            RunDto {
                id: "run-2".into(),
                started_at: 1_787_734_961_000,
                handler_id: "normalize-created-task".into(),
                outcome: "failed".into(),
                duration_ms: 240,
                error_summary: Some("Task not found (404)".into()),
            },
            RunDto {
                id: "run-1".into(),
                started_at: 1_787_734_032_000,
                handler_id: "normalize-created-task".into(),
                outcome: "timeout".into(),
                duration_ms: 30_000,
                error_summary: None,
            },
        ];
        Self {
            inner: Mutex::new(SettingsData { installation, runs }),
        }
    }

    /// Snapshot the current installation plus newest-first run history.
    pub fn snapshot(&self) -> SettingsSnapshot {
        let mut data = self.inner.lock().expect("settings lock");
        data.runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        SettingsSnapshot {
            installation: data.installation.clone(),
            runs: data.runs.clone(),
        }
    }

    /// Set the master enable switch. Returns false if no state exists.
    pub fn set_enabled(&self, enabled: bool) -> bool {
        match self.inner.lock() {
            Ok(mut data) => {
                data.installation.enabled = enabled;
                true
            }
            Err(_) => false,
        }
    }

    /// Pause or resume one handler by id. Returns false when the handler is
    /// unknown or no state exists.
    pub fn set_handler_paused(&self, handler_id: &str, paused: bool) -> bool {
        match self.inner.lock() {
            Ok(mut data) => {
                let Some(handler) = data
                    .installation
                    .handlers
                    .iter_mut()
                    .find(|handler| handler.id == handler_id)
                else {
                    return false;
                };
                handler.paused = paused;
                true
            }
            Err(_) => false,
        }
    }

    /// Append one started-run record derived from a completed invocation.
    ///
    /// `event_type` resolves to a configured handler id when possible so run
    /// history matches the fixture's per-handler view; unknown events fall
    /// back to the raw event type. Skipped invocations never start a run and
    /// are ignored by contract.
    pub fn record_run(&self, outcome: &ServerPluginOutcome, event_type: &str) {
        let Ok(mut data) = self.inner.lock() else {
            return;
        };
        let (id, duration_ms, outcome_name, error_summary) = match outcome {
            ServerPluginOutcome::Completed {
                run_id,
                duration_ms,
            } => (run_id, *duration_ms, "completed", None),
            ServerPluginOutcome::Failed {
                run_id,
                duration_ms,
                error,
            } => (run_id, *duration_ms, "failed", Some(error.message.clone())),
            ServerPluginOutcome::Timeout {
                run_id,
                duration_ms,
                error,
            } => (run_id, *duration_ms, "timeout", Some(error.message.clone())),
            // Skipped and dropped invocations never started a run, so they
            // are not recorded.
            ServerPluginOutcome::Skipped { .. } | ServerPluginOutcome::Dropped { .. } => {
                return;
            }
        };
        let handler_id = data
            .installation
            .handlers
            .iter()
            .find(|handler| handler.event == event_type)
            .map(|handler| handler.id.clone())
            .unwrap_or_else(|| event_type.to_string());
        data.runs.push(RunDto {
            id: id.clone(),
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|since| since.as_millis() as u64)
                .unwrap_or_default(),
            handler_id,
            outcome: outcome_name.into(),
            duration_ms,
            error_summary,
        });
    }
}
