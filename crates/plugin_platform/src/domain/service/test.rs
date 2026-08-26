use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::PluginPlatformService;
use crate::{
    domain::{
        errors::PluginPlatformError,
        models::{
            Capability, CapabilitySet, ClientSlot, InstallRelease, MintClientGrant,
            ProjectEditAuthorization, ProjectViewAuthorization, PublishRelease, ReleaseBundle,
            SetInstallationEnabled,
        },
        ports::PluginPlatformUseCases,
    },
    outbound::in_memory::{
        FixedClock, MemoryBundleStore, MemoryClientAuthority, MemoryInstallationRepository,
        MemoryReleaseRepository, SequenceIdGenerator, TestClientGrantSigner,
    },
};

type Service = PluginPlatformService<
    MemoryReleaseRepository,
    MemoryInstallationRepository,
    MemoryBundleStore,
    MemoryClientAuthority,
    FixedClock,
    SequenceIdGenerator,
    TestClientGrantSigner,
>;

struct Harness {
    service: Service,
    authority: MemoryClientAuthority,
}

fn harness() -> Harness {
    let authority = MemoryClientAuthority::default();
    let service = PluginPlatformService::new(
        MemoryReleaseRepository::default(),
        MemoryInstallationRepository::default(),
        MemoryBundleStore::default(),
        authority.clone(),
        FixedClock(Utc.with_ymd_and_hms(2026, 8, 26, 12, 0, 0).unwrap()),
        SequenceIdGenerator::default(),
        TestClientGrantSigner,
    );
    Harness { service, authority }
}

fn capability_set(values: &[Capability]) -> CapabilitySet {
    CapabilitySet::new(values.iter().copied())
}

fn bundle(version: &str, integrity_marker: &str) -> ReleaseBundle {
    let manifest = json!({
        "apiVersion": "1",
        "plugin": { "id": "com.macro.task-inbox", "name": "Task Inbox", "version": version },
        "capabilities": ["activity.read", "documents.read", "task-events.read", "tasks.create", "tasks.read", "tasks.rename"],
        "entrypoints": {
            "inbox": {
                "target": "client",
                "file": "client/inbox/index.js",
                "integrity": "sha256-client",
                "capabilities": ["task-events.read", "tasks.create", "tasks.read"]
            },
            "task-context": {
                "target": "client",
                "file": "client/task-context/index.js",
                "integrity": "sha256-context",
                "capabilities": ["activity.read", "documents.read", "tasks.read"]
            },
            "normalize": {
                "target": "server",
                "file": "server/normalize/index.js",
                "integrity": "sha256-server",
                "capabilities": ["tasks.read", "tasks.rename"]
            }
        },
        "slots": [
            { "entrypoint": "inbox", "slot": "project.page", "entityTypes": [] },
            { "entrypoint": "task-context", "slot": "entity.side_panel", "entityTypes": ["task"] }
        ],
        "events": [{ "entrypoint": "normalize", "event": "task.created", "delivery": "best-effort" }]
    });
    let manifest = format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()).into_bytes();
    ReleaseBundle {
        manifest,
        integrity: format!(
            r#"{{"schemaVersion":1,"marker":"{integrity_marker}"}}
"#
        )
        .into_bytes(),
        provenance: br#"{"schemaVersion":1}
"#
        .to_vec(),
        files: BTreeMap::from([
            ("client/inbox/index.js".into(), b"client".to_vec()),
            ("client/task-context/index.js".into(), b"context".to_vec()),
            ("server/normalize/index.js".into(), b"server".to_vec()),
        ]),
    }
}

async fn published_and_installed(
    harness: &Harness,
) -> (
    crate::domain::models::PluginRelease,
    crate::domain::models::PluginInstallation,
) {
    let release = harness
        .service
        .publish_release(PublishRelease {
            bundle: bundle("0.1.0", "a"),
        })
        .await
        .unwrap();
    let installation = harness
        .service
        .install_release(InstallRelease {
            authorization: ProjectEditAuthorization::new("project-1", "installer-user"),
            release_id: release.id,
        })
        .await
        .unwrap();
    (release, installation)
}

fn grant(
    installation_id: crate::domain::models::PluginInstallationId,
    viewer: &str,
) -> MintClientGrant {
    MintClientGrant {
        authorization: ProjectViewAuthorization::new("project-1", viewer),
        installation_id,
        entrypoint_id: "inbox".into(),
        slot: ClientSlot::ProjectPage,
        audience: "macro-plugin-client-v1".into(),
        lifetime_seconds: 300,
    }
}

#[tokio::test]
async fn publishes_installs_lists_gets_and_mints_viewer_bound_grant() {
    let harness = harness();
    harness.authority.set(
        "viewer-user",
        "project-1",
        capability_set(&[
            Capability::TasksRead,
            Capability::TasksCreate,
            Capability::TaskEventsRead,
        ]),
    );
    let (release, installation) = published_and_installed(&harness).await;

    let listed = harness
        .service
        .list_installations(ProjectViewAuthorization::new("project-1", "viewer-user"))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].contributions.len(), 2);
    let got = harness
        .service
        .get_installation(
            ProjectViewAuthorization::new("project-1", "viewer-user"),
            installation.id,
        )
        .await
        .unwrap();
    assert_eq!(got.release.id, release.id);

    let minted = harness
        .service
        .mint_client_grant(grant(installation.id, "viewer-user"))
        .await
        .unwrap();
    assert_eq!(minted.claims.viewer_user_id, "viewer-user");
    assert_ne!(
        minted.claims.viewer_user_id,
        installation.installed_by_user_id
    );
    assert_eq!(minted.claims.project_id, "project-1");
    assert_eq!(minted.claims.release_id, release.id);
    assert_eq!(minted.claims.entrypoint_id, "inbox");
    assert_eq!(minted.claims.audience, "macro-plugin-client-v1");
    assert_eq!(minted.claims.grant_version, 1);
    assert!(minted.token.starts_with("test-client-grant:"));
}

#[tokio::test]
async fn selecting_another_release_keeps_installation_identity_and_revokes_grants() {
    let harness = harness();
    let (_, initial) = published_and_installed(&harness).await;
    let next_release = harness
        .service
        .publish_release(PublishRelease {
            bundle: bundle("0.2.0", "b"),
        })
        .await
        .unwrap();
    let selected = harness
        .service
        .install_release(InstallRelease {
            authorization: ProjectEditAuthorization::new("project-1", "second-installer"),
            release_id: next_release.id,
        })
        .await
        .unwrap();
    assert_eq!(selected.id, initial.id);
    assert_eq!(selected.release_id, next_release.id);
    assert_eq!(selected.installed_by_user_id, "second-installer");
    assert_eq!(selected.grant_version, 2);
}

#[tokio::test]
async fn rejects_different_bytes_for_immutable_plugin_version() {
    let harness = harness();
    harness
        .service
        .publish_release(PublishRelease {
            bundle: bundle("0.1.0", "first"),
        })
        .await
        .unwrap();
    let error = harness
        .service
        .publish_release(PublishRelease {
            bundle: bundle("0.1.0", "changed"),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PluginPlatformError::ImmutableReleaseConflict { .. }
    ));
}

#[tokio::test]
async fn disabled_installation_cannot_mint() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    harness
        .service
        .set_installation_enabled(SetInstallationEnabled {
            authorization: ProjectEditAuthorization::new("project-1", "editor"),
            installation_id: installation.id,
            enabled: false,
        })
        .await
        .unwrap();
    let error = harness
        .service
        .mint_client_grant(grant(installation.id, "viewer"))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        PluginPlatformError::InstallationDisabled(installation.id)
    );
}

#[tokio::test]
async fn read_only_viewer_gets_only_read_capabilities() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    harness.authority.set(
        "viewer",
        "project-1",
        capability_set(&[Capability::TasksRead, Capability::TaskEventsRead]),
    );
    let minted = harness
        .service
        .mint_client_grant(grant(installation.id, "viewer"))
        .await
        .unwrap();
    assert_eq!(
        minted.claims.capabilities,
        capability_set(&[Capability::TasksRead, Capability::TaskEventsRead,])
    );
    assert!(!minted.claims.capabilities.contains(Capability::TasksCreate));
}

#[tokio::test]
async fn edit_viewer_intersection_adds_declared_create_but_not_server_rename() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    harness.authority.set(
        "editor",
        "project-1",
        capability_set(&[
            Capability::TasksRead,
            Capability::TasksCreate,
            Capability::TasksRename,
            Capability::DocumentsRead,
        ]),
    );
    let minted = harness
        .service
        .mint_client_grant(grant(installation.id, "editor"))
        .await
        .unwrap();
    assert!(minted.claims.capabilities.contains(Capability::TasksRead));
    assert!(minted.claims.capabilities.contains(Capability::TasksCreate));
    assert!(!minted.claims.capabilities.contains(Capability::TasksRename));
    assert!(
        !minted
            .claims
            .capabilities
            .contains(Capability::DocumentsRead)
    );
}

#[tokio::test]
async fn permission_downgrade_is_observed_through_fresh_authority_lookup() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    harness.authority.set(
        "editor",
        "project-1",
        capability_set(&[Capability::TasksRead, Capability::TasksCreate]),
    );
    let minted = harness
        .service
        .mint_client_grant(grant(installation.id, "editor"))
        .await
        .unwrap();
    harness.authority.set(
        "editor",
        "project-1",
        capability_set(&[Capability::TasksRead]),
    );

    let error = harness
        .service
        .validate_client_grant(&minted.claims, Capability::TasksCreate)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        PluginPlatformError::CapabilityDenied("tasks.create".into())
    );
    let refreshed = harness
        .service
        .mint_client_grant(grant(installation.id, "editor"))
        .await
        .unwrap();
    assert!(
        !refreshed
            .claims
            .capabilities
            .contains(Capability::TasksCreate)
    );
}

#[tokio::test]
async fn wrong_project_or_entrypoint_is_rejected() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    let project_error = harness
        .service
        .get_installation(
            ProjectViewAuthorization::new("project-2", "viewer"),
            installation.id,
        )
        .await
        .unwrap_err();
    assert_eq!(
        project_error,
        PluginPlatformError::InstallationNotFound(installation.id)
    );

    let mut request = grant(installation.id, "viewer");
    request.entrypoint_id = "missing".into();
    let entry_error = harness
        .service
        .mint_client_grant(request)
        .await
        .unwrap_err();
    assert_eq!(entry_error, PluginPlatformError::EntrypointMismatch);

    let mut request = grant(installation.id, "viewer");
    request.slot = ClientSlot::EntitySidePanel;
    let slot_error = harness
        .service
        .mint_client_grant(request)
        .await
        .unwrap_err();
    assert_eq!(slot_error, PluginPlatformError::EntrypointMismatch);
}

#[tokio::test]
async fn grant_version_change_revokes_old_claims() {
    let harness = harness();
    let (_, installation) = published_and_installed(&harness).await;
    harness.authority.set(
        "viewer",
        "project-1",
        capability_set(&[Capability::TasksRead]),
    );
    let minted = harness
        .service
        .mint_client_grant(grant(installation.id, "viewer"))
        .await
        .unwrap();
    for enabled in [false, true] {
        harness
            .service
            .set_installation_enabled(SetInstallationEnabled {
                authorization: ProjectEditAuthorization::new("project-1", "editor"),
                installation_id: installation.id,
                enabled,
            })
            .await
            .unwrap();
    }
    let error = harness
        .service
        .validate_client_grant(&minted.claims, Capability::TasksRead)
        .await
        .unwrap_err();
    assert_eq!(error, PluginPlatformError::StaleGrant);
}
