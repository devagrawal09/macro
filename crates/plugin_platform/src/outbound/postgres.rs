//! PostgreSQL repositories for releases and installations.
#![allow(
    clippy::disallowed_methods,
    reason = "POC adapter remains runtime-checked until the root-only SQLx prepare environment is available"
)]
//!
//! These queries live in the outbound adapter. The domain remains independent
//! of SQLx and receives only typed models or [`PortError`](crate::domain::ports::PortError).

#[cfg(test)]
mod test;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    models::{
        BundleObjectRef, PluginId, PluginInstallation, PluginInstallationId, PluginManifest,
        PluginRelease, PluginReleaseId, PluginVersion, ReleaseArtifactRefs,
    },
    ports::{
        InsertReleaseOutcome, InstallationRepository, InstallationSelection, PortError,
        ReleaseRepository,
    },
};

/// SQLx-backed release and installation repositories using the shared MacroDB pool.
#[derive(Debug, Clone)]
pub struct PgPluginPlatformRepository {
    pool: PgPool,
}

impl PgPluginPlatformRepository {
    /// Construct the adapter from the composition root's MacroDB pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ReleaseRow {
    id: Uuid,
    plugin_id: String,
    version: String,
    manifest: serde_json::Value,
    manifest_ref: String,
    integrity_ref: String,
    provenance_ref: String,
    release_root_sha256: String,
    created_at: DateTime<Utc>,
}

impl ReleaseRow {
    fn into_domain(self) -> Result<PluginRelease, PortError> {
        let manifest = PluginManifest::from_value(self.manifest)
            .map_err(|error| PortError(error.to_string()))?;
        let plugin_id =
            PluginId::parse(self.plugin_id).map_err(|error| PortError(error.to_string()))?;
        let version =
            PluginVersion::parse(self.version).map_err(|error| PortError(error.to_string()))?;
        Ok(PluginRelease {
            id: PluginReleaseId(self.id),
            plugin_id,
            version,
            manifest,
            artifacts: ReleaseArtifactRefs {
                manifest: BundleObjectRef(self.manifest_ref),
                integrity: BundleObjectRef(self.integrity_ref),
                provenance: BundleObjectRef(self.provenance_ref),
            },
            release_root_sha256: self.release_root_sha256,
            created_at: self.created_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct InstallationRow {
    id: Uuid,
    project_id: String,
    plugin_id: String,
    release_id: Uuid,
    installed_by_user_id: String,
    enabled: bool,
    grant_version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl InstallationRow {
    fn into_domain(self) -> Result<PluginInstallation, PortError> {
        Ok(PluginInstallation {
            id: PluginInstallationId(self.id),
            project_id: self.project_id,
            plugin_id: PluginId::parse(self.plugin_id)
                .map_err(|error| PortError(error.to_string()))?,
            release_id: PluginReleaseId(self.release_id),
            installed_by_user_id: self.installed_by_user_id,
            enabled: self.enabled,
            grant_version: self.grant_version,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

const RELEASE_COLUMNS: &str = r#"
    id, plugin_id, version, manifest, manifest_ref, integrity_ref,
    provenance_ref, release_root_sha256, created_at
"#;
const INSTALLATION_COLUMNS: &str = r#"
    id, project_id, plugin_id, release_id, installed_by_user_id,
    enabled, grant_version, created_at, updated_at
"#;

fn db(error: sqlx::Error) -> PortError {
    PortError(error.to_string())
}

#[async_trait]
impl ReleaseRepository for PgPluginPlatformRepository {
    async fn find_by_identity(
        &self,
        plugin_id: &PluginId,
        version: &PluginVersion,
    ) -> Result<Option<PluginRelease>, PortError> {
        let query = format!(
            "SELECT {RELEASE_COLUMNS} FROM plugin_releases WHERE plugin_id = $1 AND version = $2"
        );
        sqlx::query_as::<_, ReleaseRow>(&query)
            .bind(plugin_id.as_str())
            .bind(version.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .map(ReleaseRow::into_domain)
            .transpose()
    }

    async fn get_release(&self, id: PluginReleaseId) -> Result<Option<PluginRelease>, PortError> {
        let query = format!("SELECT {RELEASE_COLUMNS} FROM plugin_releases WHERE id = $1");
        sqlx::query_as::<_, ReleaseRow>(&query)
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .map(ReleaseRow::into_domain)
            .transpose()
    }

    async fn insert_release(
        &self,
        release: PluginRelease,
    ) -> Result<InsertReleaseOutcome, PortError> {
        let query = format!(
            r#"
            INSERT INTO plugin_releases (
                id, plugin_id, version, manifest, manifest_ref, integrity_ref,
                provenance_ref, release_root_sha256, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (plugin_id, version) DO NOTHING
            RETURNING {RELEASE_COLUMNS}
        "#
        );
        let inserted = sqlx::query_as::<_, ReleaseRow>(&query)
            .bind(release.id.0)
            .bind(release.plugin_id.as_str())
            .bind(release.version.as_str())
            .bind(release.manifest.raw())
            .bind(&release.artifacts.manifest.0)
            .bind(&release.artifacts.integrity.0)
            .bind(&release.artifacts.provenance.0)
            .bind(&release.release_root_sha256)
            .bind(release.created_at)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        if let Some(inserted) = inserted {
            return Ok(InsertReleaseOutcome::Inserted(inserted.into_domain()?));
        }
        let existing = self
            .find_by_identity(&release.plugin_id, &release.version)
            .await?
            .ok_or_else(|| PortError("release conflict row disappeared".into()))?;
        Ok(InsertReleaseOutcome::Existing(existing))
    }
}

#[async_trait]
impl InstallationRepository for PgPluginPlatformRepository {
    async fn install_or_select(
        &self,
        selection: InstallationSelection,
    ) -> Result<PluginInstallation, PortError> {
        let query = format!(
            r#"
            INSERT INTO plugin_installations (
                id, project_id, plugin_id, release_id, installed_by_user_id,
                enabled, grant_version, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, TRUE, 1, $6, $6)
            ON CONFLICT (project_id, plugin_id) DO UPDATE SET
                release_id = EXCLUDED.release_id,
                installed_by_user_id = EXCLUDED.installed_by_user_id,
                enabled = TRUE,
                updated_at = CASE
                    WHEN plugin_installations.release_id IS DISTINCT FROM EXCLUDED.release_id
                      OR plugin_installations.installed_by_user_id IS DISTINCT FROM EXCLUDED.installed_by_user_id
                      OR plugin_installations.enabled IS DISTINCT FROM TRUE
                    THEN EXCLUDED.updated_at
                    ELSE plugin_installations.updated_at
                END,
                grant_version = CASE
                    WHEN plugin_installations.release_id IS DISTINCT FROM EXCLUDED.release_id
                      OR plugin_installations.installed_by_user_id IS DISTINCT FROM EXCLUDED.installed_by_user_id
                      OR plugin_installations.enabled IS DISTINCT FROM TRUE
                    THEN plugin_installations.grant_version + 1
                    ELSE plugin_installations.grant_version
                END
            RETURNING {INSTALLATION_COLUMNS}
        "#
        );
        let row = sqlx::query_as::<_, InstallationRow>(&query)
            .bind(selection.new_id.0)
            .bind(&selection.project_id)
            .bind(selection.plugin_id.as_str())
            .bind(selection.release_id.0)
            .bind(&selection.installed_by_user_id)
            .bind(selection.now)
            .fetch_one(&self.pool)
            .await
            .map_err(db)?;
        row.into_domain()
    }

    async fn get_installation(
        &self,
        id: PluginInstallationId,
    ) -> Result<Option<PluginInstallation>, PortError> {
        let query =
            format!("SELECT {INSTALLATION_COLUMNS} FROM plugin_installations WHERE id = $1");
        sqlx::query_as::<_, InstallationRow>(&query)
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .map(InstallationRow::into_domain)
            .transpose()
    }

    async fn list_installations(
        &self,
        project_id: &str,
    ) -> Result<Vec<PluginInstallation>, PortError> {
        let query = format!(
            "SELECT {INSTALLATION_COLUMNS} FROM plugin_installations WHERE project_id = $1 ORDER BY plugin_id"
        );
        sqlx::query_as::<_, InstallationRow>(&query)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
            .map_err(db)?
            .into_iter()
            .map(InstallationRow::into_domain)
            .collect()
    }

    async fn set_enabled(
        &self,
        id: PluginInstallationId,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<PluginInstallation>, PortError> {
        let query = format!(
            r#"
            UPDATE plugin_installations SET
                enabled = $2,
                grant_version = CASE WHEN enabled IS DISTINCT FROM $2 THEN grant_version + 1 ELSE grant_version END,
                updated_at = CASE WHEN enabled IS DISTINCT FROM $2 THEN $3 ELSE updated_at END
            WHERE id = $1
            RETURNING {INSTALLATION_COLUMNS}
        "#
        );
        sqlx::query_as::<_, InstallationRow>(&query)
            .bind(id.0)
            .bind(enabled)
            .bind(now)
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?
            .map(InstallationRow::into_domain)
            .transpose()
    }
}
