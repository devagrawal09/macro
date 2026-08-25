//! Bounded in-memory broadcast adapter for raw live task events.

#[cfg(test)]
mod test;

use std::sync::Arc;

use crate::domain::task_events::{
    TaskEvent, TaskEventPublishError, TaskEventPublisher, TaskEventSource, TaskEventStream,
};

/// A bounded, process-local task event bus with no replay.
#[derive(Clone)]
pub struct InMemoryTaskEventBus {
    sender: Arc<tokio::sync::broadcast::Sender<TaskEvent>>,
}

impl InMemoryTaskEventBus {
    /// Construct a bus with the given bounded receiver capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
        }
    }
}

impl TaskEventPublisher for InMemoryTaskEventBus {
    fn publish(&self, event: TaskEvent) -> Result<(), TaskEventPublishError> {
        // A live-only feed intentionally drops events when nobody is listening.
        match self.sender.send(event) {
            Ok(_) | Err(tokio::sync::broadcast::error::SendError(_)) => Ok(()),
        }
    }
}

impl TaskEventSource for InMemoryTaskEventBus {
    fn subscribe(&self) -> TaskEventStream {
        let receiver = self.sender.subscribe();
        Box::pin(futures::stream::unfold(
            receiver,
            |mut receiver| async move {
                match receiver.recv().await {
                    Ok(event) => Some((event, receiver)),
                    // Lag and closure terminate this single live subscription.
                    Err(_) => None,
                }
            },
        ))
    }
}
