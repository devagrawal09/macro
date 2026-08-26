//! Small deterministic in-memory adapters for domain tests and local POC composition.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    models::{
        BundleObjectRef, CapabilitySet, ClientAuthority, ClientGrantClaims, ClientGrantId,
        PluginId, PluginInstallation, PluginInstallationId, PluginRelease, PluginReleaseId,
        PluginVersion, ReleaseArtifactRefs, ReleaseBundle,
    },
    ports::{
        BundleStore, ClientAuthorityPort, ClientGrantSigner, Clock, IdGenerator,
        InsertReleaseOutcome, InstallationRepository, InstallationSelection, PortError,
        ReleaseRepository,
    },
};

fn locked<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, PortError> {
    mutex
        .lock()
        .map_err(|_| PortError("in-memory adapter lock poisoned".into()))
}

/// In-memory immutable release repository.
#[derive(Debug, Clone, Default)]
pub struct MemoryReleaseRepository {
    state: Arc<Mutex<ReleaseState>>,
}
#[derive(Debug, Default)]
struct ReleaseState {
    by_id: HashMap<PluginReleaseId, PluginRelease>,
    identities: BTreeMap<(PluginId, PluginVersion), PluginReleaseId>,
}

#[async_trait]
impl ReleaseRepository for MemoryReleaseRepository {
    async fn find_by_identity(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<Option<PluginRelease>, PortError> {
        let state = locked(&self.state)?;
        Ok(state
            .identities
            .get(&(plugin_id.clone(), version.clone()))
            .and_then(|id| state.by_id.get(id))
            .cloned())
    }

    async fn get_release(&self, id: PluginReleaseId) -> Result<Option<PluginRelease>, PortError> {
        Ok(locked(&self.state)?.by_id.get(&id).cloned())
    }

    async fn insert_release(
        &self,
        release: PluginRelease,
    ) -> Result<InsertReleaseOutcome, PortError> {
        let mut state = locked(&self.state)?;
        let identity = (release.plugin_id.clone(), release.version.clone());
        if let Some(id) = state.identities.get(&identity) {
            let existing = state
                .by_id
                .get(id)
                .cloned()
                .ok_or_else(|| PortError("release identity index is inconsistent".into()))?;
            return Ok(InsertReleaseOutcome::Existing(existing));
        }
        state.identities.insert(identity, release.id);
        state.by_id.insert(release.id, release.clone());
        Ok(InsertReleaseOutcome::Inserted(release))
    }
}

/// In-memory project installation repository with atomic state transitions.
#[derive(Debug, Clone, Default)]
pub struct MemoryInstallationRepository {
    state: Arc<Mutex<InstallationState>>,
}
#[derive(Debug, Default)]
struct InstallationState {
    by_id: HashMap<PluginInstallationId, PluginInstallation>,
    identities: BTreeMap<(String, PluginId), PluginInstallationId>,
}

#[async_trait]
impl InstallationRepository for MemoryInstallationRepository {
    async fn install_or_select(
        &self,
        selection: InstallationSelection,
    ) -> Result<PluginInstallation, PortError> {
        let mut state = locked(&self.state)?;
        let identity = (selection.project_id.clone(), selection.plugin_id.clone());
        if let Some(id) = state.identities.get(&identity).copied() {
            let installation = state
                .by_id
                .get_mut(&id)
                .ok_or_else(|| PortError("installation identity index is inconsistent".into()))?;
            if installation.release_id != selection.release_id
                || installation.installed_by_user_id != selection.installed_by_user_id
                || !installation.enabled
            {
                installation.release_id = selection.release_id;
                installation.installed_by_user_id = selection.installed_by_user_id;
                installation.enabled = true;
                installation.grant_version += 1;
                installation.updated_at = selection.now;
            }
            return Ok(installation.clone());
        }
        let installation = PluginInstallation {
            id: selection.new_id,
            project_id: selection.project_id,
            plugin_id: selection.plugin_id,
            release_id: selection.release_id,
            installed_by_user_id: selection.installed_by_user_id,
            enabled: true,
            grant_version: 1,
            created_at: selection.now,
            updated_at: selection.now,
        };
        state.identities.insert(identity, installation.id);
        state.by_id.insert(installation.id, installation.clone());
        Ok(installation)
    }

    async fn get_installation(
        &self,
        id: PluginInstallationId,
    ) -> Result<Option<PluginInstallation>, PortError> {
        Ok(locked(&self.state)?.by_id.get(&id).cloned())
    }

    async fn list_installations(
        &self,
        project_id: &str,
    ) -> Result<Vec<PluginInstallation>, PortError> {
        let state = locked(&self.state)?;
        let mut result: Vec<_> = state
            .by_id
            .values()
            .filter(|item| item.project_id == project_id)
            .cloned()
            .collect();
        result.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        Ok(result)
    }

    async fn set_enabled(
        &self,
        id: PluginInstallationId,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<PluginInstallation>, PortError> {
        let mut state = locked(&self.state)?;
        let Some(installation) = state.by_id.get_mut(&id) else {
            return Ok(None);
        };
        if installation.enabled != enabled {
            installation.enabled = enabled;
            installation.grant_version += 1;
            installation.updated_at = now;
        }
        Ok(Some(installation.clone()))
    }
}

/// In-memory immutable bundle store.
#[derive(Debug, Clone, Default)]
pub struct MemoryBundleStore {
    releases: Arc<Mutex<HashMap<PluginReleaseId, ReleaseBundle>>>,
}

impl MemoryBundleStore {
    /// Read a stored bundle for focused adapter assertions.
    pub fn get(&self, id: PluginReleaseId) -> Option<ReleaseBundle> {
        self.releases.lock().ok()?.get(&id).cloned()
    }
}

#[async_trait]
impl BundleStore for MemoryBundleStore {
    async fn put_release(
        &self,
        release_id: PluginReleaseId,
        bundle: ReleaseBundle,
    ) -> Result<ReleaseArtifactRefs, PortError> {
        let mut releases = locked(&self.releases)?;
        if releases.insert(release_id, bundle).is_some() {
            return Err(PortError(format!("bundle {release_id} already exists")));
        }
        let prefix = format!("memory://plugin-releases/{release_id}");
        Ok(ReleaseArtifactRefs {
            manifest: BundleObjectRef(format!("{prefix}/manifest.json")),
            integrity: BundleObjectRef(format!("{prefix}/integrity.json")),
            provenance: BundleObjectRef(format!("{prefix}/provenance.json")),
        })
    }
}

/// Mutable in-memory source of fresh viewer authority.
#[derive(Debug, Clone, Default)]
pub struct MemoryClientAuthority {
    authorities: Arc<Mutex<HashMap<(String, String), CapabilitySet>>>,
}

impl MemoryClientAuthority {
    /// Replace a viewer's current authority for one project.
    pub fn set(
        &self,
        viewer_user_id: impl Into<String>,
        project_id: impl Into<String>,
        capabilities: CapabilitySet,
    ) {
        if let Ok(mut values) = self.authorities.lock() {
            values.insert((viewer_user_id.into(), project_id.into()), capabilities);
        }
    }
}

#[async_trait]
impl ClientAuthorityPort for MemoryClientAuthority {
    async fn authority(
        &self,
        viewer_user_id: &str,
        project_id: &str,
    ) -> Result<ClientAuthority, PortError> {
        let capabilities = locked(&self.authorities)?
            .get(&(viewer_user_id.to_string(), project_id.to_string()))
            .cloned()
            .unwrap_or_default();
        Ok(ClientAuthority {
            viewer_user_id: viewer_user_id.to_string(),
            project_id: project_id.to_string(),
            capabilities,
        })
    }
}

/// Fixed clock for local and domain tests.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

/// Deterministic UUID generator for local and domain tests.
#[derive(Debug, Clone, Default)]
pub struct SequenceIdGenerator {
    next: Arc<AtomicU64>,
}
impl SequenceIdGenerator {
    fn next(&self) -> Uuid {
        Uuid::from_u128(self.next.fetch_add(1, Ordering::Relaxed) as u128 + 1)
    }
}
impl IdGenerator for SequenceIdGenerator {
    fn release_id(&self) -> PluginReleaseId {
        PluginReleaseId(self.next())
    }
    fn installation_id(&self) -> PluginInstallationId {
        PluginInstallationId(self.next())
    }
    fn client_grant_id(&self) -> ClientGrantId {
        ClientGrantId(self.next())
    }
}

/// Test signer that returns a stable opaque token without cryptographic claims.
#[derive(Debug, Clone, Copy, Default)]
pub struct TestClientGrantSigner;
#[async_trait]
impl ClientGrantSigner for TestClientGrantSigner {
    async fn sign_client_grant(&self, claims: &ClientGrantClaims) -> Result<String, PortError> {
        Ok(format!("test-client-grant:{}", claims.grant_id))
    }
}
