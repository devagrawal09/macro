use std::sync::{Arc, Mutex};

use entity_access::domain::models::EntityType;
use futures::StreamExt;

use super::*;

struct FixedSource(Mutex<Vec<TaskEvent>>);

impl TaskEventSource for FixedSource {
    fn subscribe(&self) -> TaskEventStream {
        let events = self.0.lock().unwrap().clone();
        Box::pin(futures::stream::iter(events))
    }
}

fn receipt(project_id: &str) -> EntityAccessReceipt<ViewAccessLevel> {
    EntityAccessReceipt::dangerously_assert_internal_user(project_id, EntityType::Project)
}

#[tokio::test]
async fn filters_using_project_id_from_receipt() {
    let source = Arc::new(FixedSource(Mutex::new(vec![
        TaskEvent::new(TaskEventType::TaskCreated, "doc-a", "project-a"),
        TaskEvent::new(TaskEventType::TaskUpdated, "doc-b", "project-b"),
    ])));
    let service = ReceiptFilteredTaskEventSubscription::new(source);

    let events: Vec<_> = service.subscribe(receipt("project-b")).collect().await;

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].document_id, "doc-b");
}

#[test]
fn event_ids_are_distinct_uuid_v4_values() {
    let first = TaskEvent::new(TaskEventType::TaskCreated, "doc", "project");
    let second = TaskEvent::new(TaskEventType::TaskUpdated, "doc", "project");

    for event in [&first, &second] {
        assert!(!event.event_id.is_empty());
        let id = uuid::Uuid::parse_str(&event.event_id).unwrap();
        assert_eq!(id.get_version(), Some(uuid::Version::Random));
    }
    assert_ne!(first.event_id, second.event_id);
}

#[test]
fn serialized_shape_has_exact_snake_case_fields_and_closed_event_type() {
    let event = TaskEvent {
        event_id: "event".into(),
        event_type: TaskEventType::TaskCreated,
        document_id: "document".into(),
        project_id: "project".into(),
    };
    assert_eq!(
        serde_json::to_value(event).unwrap(),
        serde_json::json!({
            "event_id": "event",
            "event_type": "task.created",
            "document_id": "document",
            "project_id": "project"
        })
    );
}
