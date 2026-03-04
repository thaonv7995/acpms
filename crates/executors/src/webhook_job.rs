use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Webhook job priority for queue processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum WebhookPriority {
    High = 1, // Critical events (MR merged, pipeline failed)
    #[default]
    Normal = 5, // Standard events (push, MR updated)
    Low = 10, // Background sync operations
}

/// Represents a webhook event to be processed asynchronously
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookJob {
    pub webhook_event_id: Uuid,
    pub project_id: Uuid,
    pub event_id: String, // GitLab event ID for deduplication
    pub event_type: String,
    pub payload: serde_json::Value,
    pub attempt: u32,
    pub priority: WebhookPriority,
}

impl WebhookJob {
    pub fn new(
        webhook_event_id: Uuid,
        project_id: Uuid,
        event_id: String,
        event_type: String,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            webhook_event_id,
            project_id,
            event_id,
            event_type,
            payload,
            attempt: 0,
            priority: WebhookPriority::default(),
        }
    }

    pub fn with_priority(mut self, priority: WebhookPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }
}
