//! Errors returned by Plugin Platform use cases.

use crate::domain::models::{PluginInstallationId, PluginReleaseId};

/// A failure at the Plugin Platform domain boundary.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PluginPlatformError {
    /// A compiler artifact has an unsupported or inconsistent normal shape.
    #[error("invalid plugin artifact: {0}")]
    InvalidArtifact(String),
    /// A plugin release identity already exists with different integrity.
    #[error(
        "plugin release {plugin_id}@{version} is immutable and already has different integrity"
    )]
    ImmutableReleaseConflict {
        /// Plugin identifier involved in the conflict.
        plugin_id: String,
        /// Semantic version involved in the conflict.
        version: String,
    },
    /// A release could not be found.
    #[error("plugin release {0} was not found")]
    ReleaseNotFound(PluginReleaseId),
    /// An installation could not be found in the authorized project.
    #[error("plugin installation {0} was not found")]
    InstallationNotFound(PluginInstallationId),
    /// The installation is disabled.
    #[error("plugin installation {0} is disabled")]
    InstallationDisabled(PluginInstallationId),
    /// A requested entrypoint or slot does not belong to the selected release.
    #[error("plugin entrypoint does not match the installation release, target, or slot")]
    EntrypointMismatch,
    /// The supplied project authorization does not match the requested project.
    #[error("plugin project scope does not match the authorization")]
    ProjectMismatch,
    /// A client grant is stale or does not match current installation state.
    #[error("plugin client grant is stale or has invalid installation bindings")]
    StaleGrant,
    /// A requested capability is not present in the effective client grant.
    #[error("plugin client grant does not include {0}")]
    CapabilityDenied(String),
    /// A replaceable outbound port failed.
    #[error("plugin platform dependency failed: {0}")]
    Dependency(String),
}
