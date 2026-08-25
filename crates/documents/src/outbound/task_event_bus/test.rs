use futures::StreamExt;

use super::*;
use crate::domain::task_events::{TaskEventPublisher as _, TaskEventSource as _, TaskEventType};

#[tokio::test]
async fn broadcasts_raw_events_to_each_current_subscriber() {
    let bus = InMemoryTaskEventBus::new(8);
    let mut first = bus.subscribe();
    let mut second = bus.subscribe();
    let event = TaskEvent::new(TaskEventType::TaskCreated, "doc", "project");

    bus.publish(event.clone()).unwrap();

    assert_eq!(first.next().await, Some(event.clone()));
    assert_eq!(second.next().await, Some(event));
}

#[test]
fn publishing_without_receivers_is_a_successful_drop() {
    let bus = InMemoryTaskEventBus::new(1);
    assert!(
        bus.publish(TaskEvent::new(TaskEventType::TaskCreated, "doc", "project"))
            .is_ok()
    );
}

#[tokio::test]
async fn lag_terminates_subscription_without_replay() {
    let bus = InMemoryTaskEventBus::new(1);
    let mut stream = bus.subscribe();
    bus.publish(TaskEvent::new(TaskEventType::TaskCreated, "one", "project"))
        .unwrap();
    bus.publish(TaskEvent::new(TaskEventType::TaskUpdated, "two", "project"))
        .unwrap();

    assert_eq!(stream.next().await, None);
}
