use chrono::{TimeZone, Utc};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use super::PgPluginPlatformRepository;
use crate::domain::{
    models::{
        BundleObjectRef, PluginId, PluginRelease, PluginReleaseId, PluginVersion,
        ReleaseArtifactRefs,
    },
    ports::{
        InsertReleaseOutcome, InstallationRepository, InstallationSelection, ReleaseRepository,
    },
};

const TEST_DATABASE_URL: &str = "postgres://postgres:postgres@127.0.0.1:55432/plugin_platform_test";
const UP: &str = include_str!(
    "../../../../macro_db_client/migrations/20260826020912_create_plugin_platform_core.up.sql"
);
const DOWN: &str = include_str!(
    "../../../../macro_db_client/migrations/20260826020912_create_plugin_platform_core.down.sql"
);

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(TEST_DATABASE_URL)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "run against the documented ephemeral Postgres on port 55432"]
async fn release_and_installation_repositories_follow_immutable_schema() {
    let pool = pool().await;
    sqlx::raw_sql(r#"CREATE TABLE "User" (id TEXT PRIMARY KEY); CREATE TABLE "Project" (id TEXT PRIMARY KEY);"#)
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(UP).execute(&pool).await.unwrap();
    sqlx::raw_sql(r#"INSERT INTO "User" (id) VALUES ('installer')"#)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(r#"INSERT INTO "Project" (id) VALUES ('project')"#)
        .execute(&pool)
        .await
        .unwrap();

    let repository = PgPluginPlatformRepository::new(pool.clone());
    let manifest_value = json!({
        "apiVersion": "1",
        "plugin": { "id": "com.macro.pg-test", "name": "PG Test", "version": "0.1.0" },
        "entrypoints": {},
        "slots": []
    });
    let release = PluginRelease {
        id: PluginReleaseId(Uuid::from_u128(1)),
        plugin_id: PluginId::parse("com.macro.pg-test").unwrap(),
        version: PluginVersion::parse("0.1.0").unwrap(),
        manifest: crate::domain::models::PluginManifest::from_value(manifest_value).unwrap(),
        artifacts: ReleaseArtifactRefs {
            manifest: BundleObjectRef("memory://manifest".into()),
            integrity: BundleObjectRef("memory://integrity".into()),
            provenance: BundleObjectRef("memory://provenance".into()),
        },
        release_root_sha256: "a".repeat(64),
        created_at: Utc.with_ymd_and_hms(2026, 8, 26, 12, 0, 0).unwrap(),
    };
    assert!(matches!(
        repository.insert_release(release.clone()).await.unwrap(),
        InsertReleaseOutcome::Inserted(_)
    ));
    assert!(matches!(
        repository.insert_release(release.clone()).await.unwrap(),
        InsertReleaseOutcome::Existing(_)
    ));

    let installation = repository
        .install_or_select(InstallationSelection {
            new_id: crate::domain::models::PluginInstallationId(Uuid::from_u128(2)),
            project_id: "project".into(),
            plugin_id: release.plugin_id.clone(),
            release_id: release.id,
            installed_by_user_id: "installer".into(),
            now: release.created_at,
        })
        .await
        .unwrap();
    assert_eq!(installation.grant_version, 1);
    let disabled = repository
        .set_enabled(installation.id, false, release.created_at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(disabled.grant_version, 2);
    assert!(!disabled.enabled);
    let selected = repository
        .install_or_select(InstallationSelection {
            new_id: crate::domain::models::PluginInstallationId(Uuid::from_u128(3)),
            project_id: "project".into(),
            plugin_id: release.plugin_id.clone(),
            release_id: release.id,
            installed_by_user_id: "installer".into(),
            now: release.created_at,
        })
        .await
        .unwrap();
    assert_eq!(selected.id, installation.id);
    assert_eq!(selected.grant_version, 3);
    assert!(selected.enabled);

    sqlx::raw_sql(DOWN).execute(&pool).await.unwrap();
    sqlx::raw_sql(r#"DROP TABLE "Project"; DROP TABLE "User";"#)
        .execute(&pool)
        .await
        .unwrap();
}
