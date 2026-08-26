use serde_json::json;

use super::{EventDirection, PluginEvent, PluginManifest, admit_custom_events};
use crate::domain::errors::PluginPlatformError;

fn manifest_json(custom_events: serde_json::Value) -> serde_json::Value {
    json!({
        "apiVersion": "1",
        "plugin": { "id": "com.macro.task-inbox", "name": "Task Inbox", "version": "1.0.0" },
        "entrypoints": {
            "inbox": {
                "target": "client",
                "file": "client/inbox/index.js",
                "integrity": "sha256-client"
            }
        },
        "customEvents": custom_events,
    })
}

#[test]
fn admits_declared_events_into_the_manifest() {
    let manifest = PluginManifest::from_value(manifest_json(json!([
        { "name": "task.approved", "direction": "both" },
        { "name": "draft-saved", "direction": "client" },
    ])))
    .unwrap();
    assert_eq!(manifest.events.len(), 2);
    assert_eq!(
        manifest.events["task.approved"].direction(),
        EventDirection::Both
    );
    assert_eq!(
        manifest.events["draft-saved"].direction(),
        EventDirection::Client
    );
    assert_eq!(manifest.events["draft-saved"].name(), "draft-saved");
}

#[test]
fn omitted_custom_events_default_to_empty() {
    let mut raw = manifest_json(json!([]));
    raw.as_object_mut().unwrap().remove("customEvents");
    let manifest = PluginManifest::from_value(raw).unwrap();
    assert!(manifest.events.is_empty());
}

#[test]
fn direction_defaults_to_both() {
    let manifest =
        PluginManifest::from_value(manifest_json(json!([{ "name": "task.approved" }]))).unwrap();
    assert_eq!(
        manifest.events["task.approved"].direction(),
        EventDirection::Both
    );
}

#[test]
fn rejects_duplicate_event_declarations() {
    let error = admit_custom_events([
        ("task.approved".to_owned(), EventDirection::Both),
        ("task.approved".to_owned(), EventDirection::Client),
    ])
    .unwrap_err();
    assert!(
        matches!(error, PluginPlatformError::InvalidArtifact(ref message) if message.contains("duplicate")),
        "{error}"
    );
}

#[test]
fn rejects_invalid_event_names() {
    for name in [
        "",
        "Task.Approved",
        "-lead",
        "trail-",
        "has space",
        "emoji rocket",
    ] {
        let error = PluginEvent::parse(name, EventDirection::Both).unwrap_err();
        assert!(
            matches!(error, PluginPlatformError::InvalidArtifact(_)),
            "{name:?}: {error}"
        );
    }
}

#[test]
fn rejects_unsupported_directions() {
    let raw = json!({ "name": "task.approved", "direction": "sideways" });
    let error = serde_json::from_value::<PluginEvent>(raw).unwrap_err();
    assert!(error.to_string().contains("sideways"), "{error}");
}

#[test]
fn deserializing_a_plugin_event_validates_the_name() {
    let event: PluginEvent =
        serde_json::from_value(json!({ "name": "task.approved", "direction": "server" })).unwrap();
    assert_eq!(event.name(), "task.approved");
    assert_eq!(event.direction(), EventDirection::Server);

    let result = serde_json::from_value::<PluginEvent>(json!({ "name": "" }));
    assert!(result.is_err());
}
