//! Driving use-case and driven infrastructure ports for Plugin Platform.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::{
    errors::PluginPlatformError,
    models::{
        Capability, ClientAuthority, ClientGrantClaims, InstallRelease, InstallationContributions,
        MintClientGrant, MintedClientGrant, PluginId, PluginInstallation, PluginInstallationId,
        PluginRelease, PluginReleaseId, PluginVersion, ProjectViewAuthorization, PublishRelease,
        ReleaseArtifactRefs, ReleaseBundle, SetInstallationEnabled,
    },
};

/// A dependency failure kept independent of SQLx, object storage, and signing libraries.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct PortError(pub String);

impl From<PortError> for PluginPlatformError {
    fn from(value: PortError) -> Self {
        Self::Dependency(value.0)
    }
}

/// Outcome of inserting an immutable release under its plugin/version identity.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertReleaseOutcome {
    /// This call inserted the release.
    Inserted(PluginRelease),
    /// A concurrent or earlier publication already owns the identity.
    Existing(PluginRelease),
}

/// Outbound persistence for immutable plugin releases.
#[async_trait]
pub trait ReleaseRepository: Send + Sync {
    /// Find one release by normalized plugin identity and version.
    async fn find_by_identity(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<Option<PluginRelease>, PortError>;

    /// Find one release by stable identifier.
    async fn get_release(&self, id: PluginReleaseId) -> Result<Option<PluginRelease>, PortError>;

    /// Insert without ever overwriting the plugin/version identity.
    async fn insert_release(
        &self,
        release: PluginRelease,
    ) -> Result<InsertReleaseOutcome, PortError>;
}

/// Atomic installation selection requested by the domain service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationSelection {
    /// ID used only when a project/plugin installation does not yet exist.
    pub new_id: PluginInstallationId,
    /// Project receiving the installation.
    pub project_id: String,
    /// Plugin identity being installed.
    pub plugin_id: PluginId,
    /// Immutable release selected now.
    pub release_id: PluginReleaseId,
    /// Authenticated editor attributable for future server delegation.
    pub installed_by_user_id: String,
    /// Change timestamp.
    pub now: DateTime<Utc>,
}

/// Outbound persistence for project-scoped plugin installations.
#[async_trait]
pub trait InstallationRepository: Send + Sync {
    /// Create or atomically select a release, bumping `grant_version` on every actual selection or installer change.
    async fn install_or_select(
        &self,
        selection: InstallationSelection,
    ) -> Result<PluginInstallation, PortError>;

    /// Find an installation by stable identifier.
    async fn get_installation(
        &self,
        id: PluginInstallationId,
    ) -> Result<Option<PluginInstallation>, PortError>;

    /// List installations bound to one project in stable plugin-id order.
    async fn list_installations(
        &self,
        project_id: &str,
    ) -> Result<Vec<PluginInstallation>, PortError>;

    /// Change master state and bump `grant_version` when the state changes.
    async fn set_enabled(
        &self,
        id: PluginInstallationId,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<PluginInstallation>, PortError>;
}

/// Immutable release-byte storage, separate from document storage.
#[async_trait]
pub trait BundleStore: Send + Sync {
    /// Store a complete release under a fresh release ID without overwriting bytes.
    async fn put_release(
        &self,
        release_id: PluginReleaseId,
        bundle: ReleaseBundle,
    ) -> Result<ReleaseArtifactRefs, PortError>;
}

/// Fresh viewer authority used for client capability intersection and revalidation.
#[async_trait]
pub trait ClientAuthorityPort: Send + Sync {
    /// Resolve the viewer's current closed operations for exactly one project.
    async fn authority(
        &self,
        viewer_user_id: &str,
        project_id: &str,
    ) -> Result<ClientAuthority, PortError>;
}

/// Source of deterministic domain time.
pub trait Clock: Send + Sync {
    /// Current UTC time.
    fn now(&self) -> DateTime<Utc>;
}

/// Source of opaque release, installation, and grant identifiers.
pub trait IdGenerator: Send + Sync {
    /// Fresh immutable release identifier.
    fn release_id(&self) -> PluginReleaseId;
    /// Fresh project installation identifier.
    fn installation_id(&self) -> PluginInstallationId;
    /// Fresh client grant identifier.
    fn client_grant_id(&self) -> crate::domain::models::ClientGrantId;
}

/// Signer for a fully derived client grant description.
#[async_trait]
pub trait ClientGrantSigner: Send + Sync {
    /// Sign exactly these claims and return an opaque token.
    async fn sign_client_grant(&self, claims: &ClientGrantClaims) -> Result<String, PortError>;
}

/// Driving API exposed by the standalone backend core to later inbound adapters.
#[async_trait]
pub trait PluginPlatformUseCases: Send + Sync {
    /// Publish or idempotently reuse one immutable compiler release.
    async fn publish_release(
        &self,
        command: PublishRelease,
    ) -> Result<PluginRelease, PluginPlatformError>;
    /// Install or select an immutable release for a Project Edit-authorized caller.
    async fn install_release(
        &self,
        command: InstallRelease,
    ) -> Result<PluginInstallation, PluginPlatformError>;
    /// Get one installation and contributions in a Project View-authorized scope.
    async fn get_installation(
        &self,
        authorization: ProjectViewAuthorization,
        installation_id: PluginInstallationId,
    ) -> Result<InstallationContributions, PluginPlatformError>;
    /// List installations and contributions in a Project View-authorized scope.
    async fn list_installations(
        &self,
        authorization: ProjectViewAuthorization,
    ) -> Result<Vec<InstallationContributions>, PluginPlatformError>;
    /// Enable or disable an installation with Project Edit authority.
    async fn set_installation_enabled(
        &self,
        command: SetInstallationEnabled,
    ) -> Result<PluginInstallation, PluginPlatformError>;
    /// Mint a fresh viewer-bound client grant description and token.
    async fn mint_client_grant(
        &self,
        command: MintClientGrant,
    ) -> Result<MintedClientGrant, PluginPlatformError>;
    /// Revalidate current state, viewer authority, and one capability for existing claims.
    async fn validate_client_grant(
        &self,
        claims: &ClientGrantClaims,
        required: Capability,
    ) -> Result<(), PluginPlatformError>;
}
