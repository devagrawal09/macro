//! Core Plugin Platform use-case implementation.

#[cfg(test)]
mod test;

use async_trait::async_trait;
use chrono::Duration;
use sha2::{Digest, Sha256};

use crate::domain::{
    errors::PluginPlatformError,
    models::{
        Capability, ClientGrantClaims, InstallRelease, InstallationContributions, MintClientGrant,
        MintedClientGrant, PluginInstallation, PluginInstallationId, PluginRelease,
        ProjectViewAuthorization, PublishRelease, SetInstallationEnabled,
    },
    ports::{
        BundleStore, ClientAuthorityPort, ClientGrantSigner, Clock, IdGenerator,
        InsertReleaseOutcome, InstallationRepository, InstallationSelection,
        PluginPlatformUseCases, ReleaseRepository,
    },
};

/// Domain service coordinating release, installation, authority, and grant ports.
#[derive(Debug)]
pub struct PluginPlatformService<R, I, B, A, C, G, S> {
    releases: R,
    installations: I,
    bundles: B,
    authority: A,
    clock: C,
    ids: G,
    signer: S,
}

impl<R, I, B, A, C, G, S> PluginPlatformService<R, I, B, A, C, G, S> {
    /// Compose the core from replaceable outbound ports.
    pub fn new(
        releases: R,
        installations: I,
        bundles: B,
        authority: A,
        clock: C,
        ids: G,
        signer: S,
    ) -> Self {
        Self {
            releases,
            installations,
            bundles,
            authority,
            clock,
            ids,
            signer,
        }
    }
}

impl<R, I, B, A, C, G, S> PluginPlatformService<R, I, B, A, C, G, S>
where
    R: ReleaseRepository,
    I: InstallationRepository,
{
    async fn installation_in_project(
        &self,
        id: PluginInstallationId,
        project_id: &str,
    ) -> Result<PluginInstallation, PluginPlatformError> {
        let installation = self
            .installations
            .get_installation(id)
            .await?
            .ok_or(PluginPlatformError::InstallationNotFound(id))?;
        if installation.project_id != project_id {
            return Err(PluginPlatformError::InstallationNotFound(id));
        }
        Ok(installation)
    }

    async fn contributions(
        &self,
        installation: PluginInstallation,
    ) -> Result<InstallationContributions, PluginPlatformError> {
        let release = self
            .releases
            .get_release(installation.release_id)
            .await?
            .ok_or(PluginPlatformError::ReleaseNotFound(
                installation.release_id,
            ))?;
        let contributions = release.manifest.contributions.clone();
        Ok(InstallationContributions {
            installation,
            release,
            contributions,
        })
    }
}

#[async_trait]
impl<R, I, B, A, C, G, S> PluginPlatformUseCases for PluginPlatformService<R, I, B, A, C, G, S>
where
    R: ReleaseRepository,
    I: InstallationRepository,
    B: BundleStore,
    A: ClientAuthorityPort,
    C: Clock,
    G: IdGenerator,
    S: ClientGrantSigner,
{
    async fn publish_release(
        &self,
        command: PublishRelease,
    ) -> Result<PluginRelease, PluginPlatformError> {
        let manifest = crate::domain::models::PluginManifest::parse(&command.bundle.manifest)?;
        let root = format!("{:x}", Sha256::digest(&command.bundle.integrity));
        if let Some(existing) = self
            .releases
            .find_by_identity(&manifest.plugin_id, &manifest.version)
            .await?
        {
            return if existing.release_root_sha256 == root {
                Ok(existing)
            } else {
                Err(PluginPlatformError::ImmutableReleaseConflict {
                    plugin_id: manifest.plugin_id.to_string(),
                    version: manifest.version.to_string(),
                })
            };
        }

        let id = self.ids.release_id();
        let artifacts = self.bundles.put_release(id, command.bundle).await?;
        let release = PluginRelease {
            id,
            plugin_id: manifest.plugin_id.clone(),
            version: manifest.version.clone(),
            manifest,
            artifacts,
            release_root_sha256: root,
            created_at: self.clock.now(),
        };
        let expected_root = release.release_root_sha256.clone();
        match self.releases.insert_release(release).await? {
            InsertReleaseOutcome::Inserted(release) => Ok(release),
            InsertReleaseOutcome::Existing(existing)
                if existing.release_root_sha256 == expected_root =>
            {
                Ok(existing)
            }
            InsertReleaseOutcome::Existing(existing) => {
                Err(PluginPlatformError::ImmutableReleaseConflict {
                    plugin_id: existing.plugin_id.to_string(),
                    version: existing.version.to_string(),
                })
            }
        }
    }

    async fn install_release(
        &self,
        command: InstallRelease,
    ) -> Result<PluginInstallation, PluginPlatformError> {
        let release = self
            .releases
            .get_release(command.release_id)
            .await?
            .ok_or(PluginPlatformError::ReleaseNotFound(command.release_id))?;
        Ok(self
            .installations
            .install_or_select(InstallationSelection {
                new_id: self.ids.installation_id(),
                project_id: command.authorization.project_id,
                plugin_id: release.plugin_id,
                release_id: release.id,
                installed_by_user_id: command.authorization.editor_user_id,
                now: self.clock.now(),
            })
            .await?)
    }

    async fn get_installation(
        &self,
        authorization: ProjectViewAuthorization,
        installation_id: PluginInstallationId,
    ) -> Result<InstallationContributions, PluginPlatformError> {
        let installation = self
            .installation_in_project(installation_id, &authorization.project_id)
            .await?;
        self.contributions(installation).await
    }

    async fn list_installations(
        &self,
        authorization: ProjectViewAuthorization,
    ) -> Result<Vec<InstallationContributions>, PluginPlatformError> {
        let installations = self
            .installations
            .list_installations(&authorization.project_id)
            .await?;
        let mut result = Vec::with_capacity(installations.len());
        for installation in installations {
            result.push(self.contributions(installation).await?);
        }
        Ok(result)
    }

    async fn set_installation_enabled(
        &self,
        command: SetInstallationEnabled,
    ) -> Result<PluginInstallation, PluginPlatformError> {
        self.installation_in_project(command.installation_id, &command.authorization.project_id)
            .await?;
        self.installations
            .set_enabled(command.installation_id, command.enabled, self.clock.now())
            .await?
            .ok_or(PluginPlatformError::InstallationNotFound(
                command.installation_id,
            ))
    }

    async fn mint_client_grant(
        &self,
        command: MintClientGrant,
    ) -> Result<MintedClientGrant, PluginPlatformError> {
        if command.audience.trim().is_empty() || !(1..=300).contains(&command.lifetime_seconds) {
            return Err(PluginPlatformError::InvalidArtifact(
                "client audience must be present and lifetime must be 1..=300 seconds".into(),
            ));
        }
        let installation = self
            .installation_in_project(command.installation_id, &command.authorization.project_id)
            .await?;
        if !installation.enabled {
            return Err(PluginPlatformError::InstallationDisabled(installation.id));
        }
        let release = self
            .releases
            .get_release(installation.release_id)
            .await?
            .ok_or(PluginPlatformError::ReleaseNotFound(
                installation.release_id,
            ))?;
        let entrypoint = release
            .manifest
            .client_entrypoint(&command.entrypoint_id, command.slot)
            .ok_or(PluginPlatformError::EntrypointMismatch)?;
        let authority = self
            .authority
            .authority(
                &command.authorization.viewer_user_id,
                &command.authorization.project_id,
            )
            .await?;
        if authority.viewer_user_id != command.authorization.viewer_user_id
            || authority.project_id != command.authorization.project_id
        {
            return Err(PluginPlatformError::ProjectMismatch);
        }
        let effective = entrypoint
            .capabilities
            .client_allowlisted()
            .intersection(&authority.capabilities);
        let issued_at = self.clock.now();
        let claims = ClientGrantClaims {
            grant_id: self.ids.client_grant_id(),
            viewer_user_id: command.authorization.viewer_user_id,
            installation_id: installation.id,
            release_id: release.id,
            project_id: installation.project_id,
            entrypoint_id: entrypoint.id.clone(),
            slot: command.slot,
            audience: command.audience,
            capabilities: effective,
            grant_version: installation.grant_version,
            release_root_sha256: release.release_root_sha256,
            issued_at,
            expires_at: issued_at + Duration::seconds(command.lifetime_seconds),
        };
        let token = self.signer.sign_client_grant(&claims).await?;
        Ok(MintedClientGrant { claims, token })
    }

    async fn validate_client_grant(
        &self,
        claims: &ClientGrantClaims,
        required: Capability,
    ) -> Result<(), PluginPlatformError> {
        if self.clock.now() >= claims.expires_at || !required.client_allowed() {
            return Err(PluginPlatformError::StaleGrant);
        }
        let installation = self
            .installations
            .get_installation(claims.installation_id)
            .await?
            .ok_or(PluginPlatformError::StaleGrant)?;
        if !installation.enabled
            || installation.project_id != claims.project_id
            || installation.release_id != claims.release_id
            || installation.grant_version != claims.grant_version
        {
            return Err(PluginPlatformError::StaleGrant);
        }
        let release = self
            .releases
            .get_release(installation.release_id)
            .await?
            .ok_or(PluginPlatformError::StaleGrant)?;
        let Some(entrypoint) = release
            .manifest
            .client_entrypoint(&claims.entrypoint_id, claims.slot)
        else {
            return Err(PluginPlatformError::StaleGrant);
        };
        if release.release_root_sha256 != claims.release_root_sha256
            || !entrypoint.capabilities.contains(required)
            || !claims.capabilities.contains(required)
        {
            return Err(PluginPlatformError::CapabilityDenied(required.to_string()));
        }
        let authority = self
            .authority
            .authority(&claims.viewer_user_id, &claims.project_id)
            .await?;
        if authority.viewer_user_id != claims.viewer_user_id
            || authority.project_id != claims.project_id
        {
            return Err(PluginPlatformError::StaleGrant);
        }
        if !authority.capabilities.contains(required) {
            return Err(PluginPlatformError::CapabilityDenied(required.to_string()));
        }
        Ok(())
    }
}
