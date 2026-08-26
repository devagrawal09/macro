//! Plugin release, installation, manifest, capability, and grant models.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::domain::errors::PluginPlatformError;

macro_rules! uuid_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
uuid_id!(
    PluginReleaseId,
    "Stable identifier for one immutable plugin release."
);
uuid_id!(
    PluginInstallationId,
    "Stable identifier for one project plugin installation."
);
uuid_id!(
    ClientGrantId,
    "Unique identifier for one minted client grant description."
);

/// A validated reverse-domain plugin identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PluginId(String);

impl PluginId {
    /// Parse the normalized compiler plugin identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, PluginPlatformError> {
        let value = value.into();
        let segments: Vec<_> = value.split('.').collect();
        let valid_segment = |segment: &&str| {
            !segment.is_empty()
                && segment.len() <= 63
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        if value.len() > 253 || segments.len() < 2 || !segments.iter().all(valid_segment) {
            return Err(PluginPlatformError::InvalidArtifact(format!(
                "plugin id {value:?} must be a lowercase reverse-domain identifier"
            )));
        }
        Ok(Self(value))
    }

    /// Return the normalized identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PluginId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A normalized semantic plugin version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PluginVersion(String);

impl PluginVersion {
    /// Parse and normalize a semantic version.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, PluginPlatformError> {
        Version::parse(value.as_ref())
            .map(|version| Self(version.to_string()))
            .map_err(|error| {
                PluginPlatformError::InvalidArtifact(format!("invalid plugin version: {error}"))
            })
    }

    /// Return the normalized semantic version.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PluginVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Closed set of capabilities understood by the first Plugin Platform client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Read tasks in the bound project.
    TasksRead,
    /// Create tasks in the bound project.
    TasksCreate,
    /// Rename tasks; reserved for later server grants.
    TasksRename,
    /// Read the live best-effort task event stream.
    TaskEventsRead,
    /// Read documents in the bound project.
    DocumentsRead,
    /// Read bounded activity summaries in the bound project.
    ActivityRead,
}

impl Capability {
    /// Stable manifest and claim spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TasksRead => "tasks.read",
            Self::TasksCreate => "tasks.create",
            Self::TasksRename => "tasks.rename",
            Self::TaskEventsRead => "task-events.read",
            Self::DocumentsRead => "documents.read",
            Self::ActivityRead => "activity.read",
        }
    }

    /// Whether this capability may enter a client grant.
    pub const fn client_allowed(self) -> bool {
        !matches!(self, Self::TasksRename)
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Capability {
    type Err = PluginPlatformError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tasks.read" => Ok(Self::TasksRead),
            "tasks.create" => Ok(Self::TasksCreate),
            "tasks.rename" => Ok(Self::TasksRename),
            "task-events.read" => Ok(Self::TaskEventsRead),
            "documents.read" => Ok(Self::DocumentsRead),
            "activity.read" => Ok(Self::ActivityRead),
            _ => Err(PluginPlatformError::InvalidArtifact(format!(
                "unsupported capability {value:?}"
            ))),
        }
    }
}

impl Serialize for Capability {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// A deterministic set of closed capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilitySet(BTreeSet<Capability>);

impl CapabilitySet {
    /// Construct a set from capabilities.
    pub fn new(values: impl IntoIterator<Item = Capability>) -> Self {
        Self(values.into_iter().collect())
    }
    /// Test set membership.
    pub fn contains(&self, value: Capability) -> bool {
        self.0.contains(&value)
    }
    /// Iterate in stable manifest spelling order.
    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.0.iter().copied()
    }
    /// Return the intersection with another set.
    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0.intersection(&other.0).copied().collect())
    }
    /// Return only capabilities allowed by the client platform.
    pub fn client_allowlisted(&self) -> Self {
        Self::new(self.iter().filter(|capability| capability.client_allowed()))
    }
}

/// Target runtime selected by the compiler for an entrypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntrypointTarget {
    /// Opaque-frame browser entrypoint.
    Client,
    /// Best-effort server entrypoint, deferred from this core checkpoint.
    Server,
}

/// Client host slot selected by a compiler contribution record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientSlot {
    /// Project-level page contribution.
    #[serde(rename = "project.page")]
    ProjectPage,
    /// Contextual entity side-panel contribution.
    #[serde(rename = "entity.side_panel")]
    EntitySidePanel,
}

impl fmt::Display for ClientSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProjectPage => "project.page",
            Self::EntitySidePanel => "entity.side_panel",
        })
    }
}

/// One compiler-produced executable entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrypointRecord {
    /// Compiler entrypoint identifier.
    pub id: String,
    /// Client or server build target.
    pub target: EntrypointTarget,
    /// Release-relative JavaScript file.
    pub file: String,
    /// Compiler-recorded SHA-256 integrity string.
    pub integrity: String,
    /// Capabilities declared for exactly this entrypoint.
    pub capabilities: CapabilitySet,
}

/// One static client contribution to a Macro host slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionRecord {
    /// Entrypoint selected for this contribution.
    pub entrypoint_id: String,
    /// Host slot selected by the compiler.
    pub slot: ClientSlot,
    /// Entity types allowed for contextual side-panel mounting.
    pub entity_types: Vec<String>,
}

/// The small typed projection of a canonical compiler manifest needed by the core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    raw: serde_json::Value,
    /// Compiler API contract version.
    pub api_version: String,
    /// Immutable plugin identifier.
    pub plugin_id: PluginId,
    /// Display name from the compiler manifest.
    pub name: String,
    /// Immutable semantic release version.
    pub version: PluginVersion,
    /// Entrypoints keyed by compiler entrypoint identifier.
    pub entrypoints: BTreeMap<String, EntrypointRecord>,
    /// Static client contribution records.
    pub contributions: Vec<ContributionRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerManifest {
    api_version: String,
    plugin: CompilerPlugin,
    entrypoints: BTreeMap<String, CompilerEntrypoint>,
    #[serde(default)]
    slots: Vec<CompilerSlot>,
}
#[derive(Deserialize)]
struct CompilerPlugin {
    id: String,
    name: String,
    version: String,
}
#[derive(Deserialize)]
struct CompilerEntrypoint {
    target: EntrypointTarget,
    file: String,
    integrity: String,
    #[serde(default)]
    capabilities: CapabilitySet,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompilerSlot {
    entrypoint: String,
    slot: ClientSlot,
    #[serde(default)]
    entity_types: Vec<String>,
}

impl PluginManifest {
    /// Parse the normal canonical JSON shape emitted by `@macro/plugin-cli`.
    ///
    /// This performs only domain consistency checks. The compiler owns source
    /// graph and hostile-input validation.
    pub fn parse(bytes: &[u8]) -> Result<Self, PluginPlatformError> {
        let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
            PluginPlatformError::InvalidArtifact(format!("manifest JSON: {error}"))
        })?;
        Self::from_value(raw)
    }

    /// Rebuild the typed projection from the canonical JSON stored in Postgres.
    pub fn from_value(raw: serde_json::Value) -> Result<Self, PluginPlatformError> {
        let compiler: CompilerManifest = serde_json::from_value(raw.clone()).map_err(|error| {
            PluginPlatformError::InvalidArtifact(format!("manifest shape: {error}"))
        })?;
        if compiler.api_version != "1" {
            return Err(PluginPlatformError::InvalidArtifact(
                "apiVersion must be 1".into(),
            ));
        }
        let plugin_id = PluginId::parse(compiler.plugin.id)?;
        let version = PluginVersion::parse(compiler.plugin.version)?;
        let mut entrypoints = BTreeMap::new();
        for (id, entry) in compiler.entrypoints {
            if id.is_empty() || entry.file.is_empty() || entry.integrity.is_empty() {
                return Err(PluginPlatformError::InvalidArtifact(
                    "entrypoint id, file, and integrity must be present".into(),
                ));
            }
            entrypoints.insert(
                id.clone(),
                EntrypointRecord {
                    id,
                    target: entry.target,
                    file: entry.file,
                    integrity: entry.integrity,
                    capabilities: entry.capabilities,
                },
            );
        }
        let mut contributions = Vec::new();
        for slot in compiler.slots {
            let entry = entrypoints.get(&slot.entrypoint).ok_or_else(|| {
                PluginPlatformError::InvalidArtifact(format!(
                    "slot references missing entrypoint {:?}",
                    slot.entrypoint
                ))
            })?;
            if entry.target != EntrypointTarget::Client {
                return Err(PluginPlatformError::InvalidArtifact(
                    "slot must reference a client entrypoint".into(),
                ));
            }
            contributions.push(ContributionRecord {
                entrypoint_id: slot.entrypoint,
                slot: slot.slot,
                entity_types: slot.entity_types,
            });
        }
        Ok(Self {
            raw,
            api_version: compiler.api_version,
            plugin_id,
            name: compiler.plugin.name,
            version,
            entrypoints,
            contributions,
        })
    }

    /// Canonical compiler JSON retained without a second serialization contract.
    pub fn raw(&self) -> &serde_json::Value {
        &self.raw
    }

    /// Find a client entrypoint mounted into the exact requested slot.
    pub fn client_entrypoint(&self, id: &str, slot: ClientSlot) -> Option<&EntrypointRecord> {
        let contribution = self
            .contributions
            .iter()
            .find(|item| item.entrypoint_id == id && item.slot == slot)?;
        let entrypoint = self.entrypoints.get(&contribution.entrypoint_id)?;
        (entrypoint.target == EntrypointTarget::Client).then_some(entrypoint)
    }
}

/// Opaque immutable object reference returned by a bundle store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BundleObjectRef(pub String);

/// Canonical references for the three release metadata files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseArtifactRefs {
    /// Canonical compiler manifest reference.
    pub manifest: BundleObjectRef,
    /// Canonical integrity manifest reference.
    pub integrity: BundleObjectRef,
    /// Canonical provenance manifest reference.
    pub provenance: BundleObjectRef,
}

/// Canonical compiler release bytes handed to a bundle store as one immutable unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseBundle {
    /// Canonical `manifest.json` bytes.
    pub manifest: Vec<u8>,
    /// Canonical `integrity.json` bytes.
    pub integrity: Vec<u8>,
    /// Canonical `provenance.json` bytes.
    pub provenance: Vec<u8>,
    /// Compiler output files keyed by release-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// One immutable published plugin release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRelease {
    /// Stable release identifier.
    pub id: PluginReleaseId,
    /// Immutable reverse-domain plugin identifier.
    pub plugin_id: PluginId,
    /// Immutable normalized semantic version.
    pub version: PluginVersion,
    /// Typed projection plus canonical compiler manifest JSON.
    pub manifest: PluginManifest,
    /// Immutable bundle-store references.
    pub artifacts: ReleaseArtifactRefs,
    /// SHA-256 hex digest of canonical `integrity.json` bytes.
    pub release_root_sha256: String,
    /// Publication time supplied by the clock port.
    pub created_at: DateTime<Utc>,
}

/// Project-scoped selection of one immutable plugin release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstallation {
    /// Stable installation identifier.
    pub id: PluginInstallationId,
    /// Bound Macro project identifier.
    pub project_id: String,
    /// Plugin identity selected in this project.
    pub plugin_id: PluginId,
    /// Currently selected immutable release.
    pub release_id: PluginReleaseId,
    /// User who performed the latest install or release selection.
    pub installed_by_user_id: String,
    /// Whether contributions and grants are enabled.
    pub enabled: bool,
    /// Monotonic revocation version bound into grants.
    pub grant_version: i64,
    /// Installation creation time.
    pub created_at: DateTime<Utc>,
    /// Latest release, installer, or enabled-state change.
    pub updated_at: DateTime<Utc>,
}

/// Typed proof that an inbound adapter established project view authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectViewAuthorization {
    /// Authorized project.
    pub project_id: String,
    /// Current viewer identity.
    pub viewer_user_id: String,
}
impl ProjectViewAuthorization {
    /// Construct from a verified transport-independent authority receipt.
    pub fn new(project_id: impl Into<String>, viewer_user_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            viewer_user_id: viewer_user_id.into(),
        }
    }
}

/// Typed proof that an inbound adapter established project edit authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEditAuthorization {
    /// Authorized project.
    pub project_id: String,
    /// Current editor identity.
    pub editor_user_id: String,
}
impl ProjectEditAuthorization {
    /// Construct from a verified transport-independent authority receipt.
    pub fn new(project_id: impl Into<String>, editor_user_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            editor_user_id: editor_user_id.into(),
        }
    }
}

/// Command to publish one compiler-produced immutable release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishRelease {
    /// Complete canonical compiler output.
    pub bundle: ReleaseBundle,
}

/// Command to install or select a release in an authorized project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRelease {
    /// Typed Project Edit authority and attributable installer.
    pub authorization: ProjectEditAuthorization,
    /// Immutable release to select.
    pub release_id: PluginReleaseId,
}

/// Command to enable or disable one installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetInstallationEnabled {
    /// Typed Project Edit authority.
    pub authorization: ProjectEditAuthorization,
    /// Installation in that project.
    pub installation_id: PluginInstallationId,
    /// New master state.
    pub enabled: bool,
}

/// Viewer authority resolved freshly for a client grant or protected call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAuthority {
    /// Viewer whose authority was resolved.
    pub viewer_user_id: String,
    /// Project in which authority was resolved.
    pub project_id: String,
    /// Closed operations currently allowed to that viewer.
    pub capabilities: CapabilitySet,
}

/// Request to mint a viewer-bound client grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintClientGrant {
    /// Typed Project View authority from the inbound edge.
    pub authorization: ProjectViewAuthorization,
    /// Enabled installation to bind.
    pub installation_id: PluginInstallationId,
    /// Client entrypoint to bind.
    pub entrypoint_id: String,
    /// Exact host slot to bind.
    pub slot: ClientSlot,
    /// Exact audience accepted by the future client runtime extractor.
    pub audience: String,
    /// Short grant lifetime in seconds.
    pub lifetime_seconds: i64,
}

/// Complete description signed into a short-lived client grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientGrantClaims {
    /// Unique grant identifier.
    pub grant_id: ClientGrantId,
    /// Current viewer; never the installer by substitution.
    pub viewer_user_id: String,
    /// Bound installation.
    pub installation_id: PluginInstallationId,
    /// Bound immutable release.
    pub release_id: PluginReleaseId,
    /// Bound project.
    pub project_id: String,
    /// Bound client entrypoint.
    pub entrypoint_id: String,
    /// Bound host slot.
    pub slot: ClientSlot,
    /// Exact client runtime audience.
    pub audience: String,
    /// Effective declared, authorized, scoped, and platform-allowlisted capabilities.
    pub capabilities: CapabilitySet,
    /// Installation revocation version at mint time.
    pub grant_version: i64,
    /// Release integrity root.
    pub release_root_sha256: String,
    /// Mint time.
    pub issued_at: DateTime<Utc>,
    /// Expiry time.
    pub expires_at: DateTime<Utc>,
}

/// Signed client grant plus its inspectable domain description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintedClientGrant {
    /// Description bound into the signature.
    pub claims: ClientGrantClaims,
    /// Opaque signed token returned by the signer port.
    pub token: String,
}

/// Installation with the selected release and its static contributions.
#[derive(Debug, Clone, PartialEq)]
pub struct InstallationContributions {
    /// Project-scoped installation.
    pub installation: PluginInstallation,
    /// Selected immutable release.
    pub release: PluginRelease,
    /// Static client contributions from that release.
    pub contributions: Vec<ContributionRecord>,
}
