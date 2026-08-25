//! Domain contracts and receipt-filtered subscription service for live task events.

#[cfg(test)]
mod test;

use std::pin::Pin;
use std::sync::Arc;

use entity_access::domain::models::{EntityAccessReceipt, ViewAccessLevel};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

/// A task lifecycle event delivered to authorized project subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(utoipa::ToSchema))]
pub struct TaskEvent {
    /// Unique UUID v4 generated for this emission.
    pub event_id: String,
    /// Closed task event kind.
    pub event_type: TaskEventType,
    /// Task document identifier.
    pub document_id: String,
    /// Project containing the task.
    pub project_id: String,
}

/// Supported task lifecycle event kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(utoipa::ToSchema))]
pub enum TaskEventType {
    /// A task completed creation and content finalization.
    #[serde(rename = "task.created")]
    TaskCreated,
    /// A task title was successfully changed without a project move.
    #[serde(rename = "task.updated")]
    TaskUpdated,
}

impl TaskEvent {
    /// Construct an event with one fresh UUID v4 identifier.
    pub fn new(
        event_type: TaskEventType,
        document_id: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type,
            document_id: document_id.into(),
            project_id: project_id.into(),
        }
    }
}

/// A boxed, process-local stream of task events.
pub type TaskEventStream = Pin<Box<dyn Stream<Item = TaskEvent> + Send>>;

/// Error returned when a live task event cannot be published.
#[derive(Debug, thiserror::Error)]
#[error("task event publish failed: {0}")]
pub struct TaskEventPublishError(pub String);

/// Outbound port for publishing raw task events.
pub trait TaskEventPublisher: Send + Sync {
    /// Publish one event. No active subscribers is a successful drop.
    fn publish(&self, event: TaskEvent) -> Result<(), TaskEventPublishError>;
}

/// Outbound port for subscribing to the raw, unfiltered live event feed.
pub trait TaskEventSource: Send + Sync {
    /// Subscribe from the current point in time. There is no replay.
    fn subscribe(&self) -> TaskEventStream;
}

/// Inbound domain service that consumes a typed project-access receipt.
pub trait TaskEventSubscriptionService: Send + Sync {
    /// Subscribe only to events for the project proven by `receipt`.
    fn subscribe(&self, receipt: EntityAccessReceipt<ViewAccessLevel>) -> TaskEventStream;
}

/// Receipt-filtering implementation of the task event subscription use case.
pub struct ReceiptFilteredTaskEventSubscription {
    source: Arc<dyn TaskEventSource>,
}

impl ReceiptFilteredTaskEventSubscription {
    /// Construct the domain service over a raw source port.
    pub fn new(source: Arc<dyn TaskEventSource>) -> Self {
        Self { source }
    }
}

impl TaskEventSubscriptionService for ReceiptFilteredTaskEventSubscription {
    fn subscribe(&self, receipt: EntityAccessReceipt<ViewAccessLevel>) -> TaskEventStream {
        let authorized_project_id = receipt.entity().entity_id.clone();
        Box::pin(
            self.source.subscribe().filter(move |event| {
                futures::future::ready(event.project_id == authorized_project_id)
            }),
        )
    }
}
