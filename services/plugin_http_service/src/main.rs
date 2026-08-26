//! Dev-only HTTP surface for the Plugin Platform core.
//!
//! Composition root: wires the Bun server-plugin runtime adapter into the
//! thin inbound router from [`api`] and serves it. This service is
//! unauthenticated and must not be exposed beyond local development.

mod api;
mod settings;

use std::sync::Arc;

use macro_env_var::maybe_env_var;
use plugin_platform::outbound::bun_runtime::BunServerPluginRuntime;
use rootcause::Report;
use rootcause::prelude::ResultExt;

maybe_env_var! {
    /// Port for the dev-only plugin HTTP service. Defaults to 8135.
    struct PluginHttpPort;
}

/// Default dev port when `PLUGIN_HTTP_PORT` is unset.
const DEFAULT_PORT: u16 = 8135;

#[tokio::main]
async fn main() -> Result<(), Report> {
    macro_entrypoint::MacroEntrypoint::default().init();

    let runtime = BunServerPluginRuntime::from_env()
        .into_report()
        .context("failed to configure the bun server-plugin runtime")?;

    let port = PluginHttpPort::new()
        .and_then(|value| value.value().map(str::to_string))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let app = api::plugin_router(api::PluginHttpState {
        runtime: Arc::new(runtime),
        settings: Some(Arc::new(settings::PluginSettingsStore::seeded())),
    });

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .into_report()
        .context("failed to bind plugin http service listener")?;
    tracing::info!("plugin http service listening on port {port}");

    axum::serve(listener, app.into_make_service())
        .await
        .into_report()
        .context("plugin http service terminated with an error")?;
    Ok(())
}
