//! Input Queue Enhancement
//!
//! Manages queued inputs for agent sessions with timeout support.
//! Allows inputs to be queued while agent is busy processing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::debug;
use uuid::Uuid;

/// A message in the input queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
    /// Unique ID for this input.
    pub id: Uuid,
    /// The input content.
    pub content: String,
    /// When the input was queued.
    pub queued_at: DateTime<Utc>,
    /// Who sent the input (user ID).
    pub sender_id: Option<Uuid>,
}

impl InputMessage {
    /// Create a new input message.
    pub fn new(content: String, sender_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            queued_at: Utc::now(),
            sender_id,
        }
    }
}

/// Input queue for managing agent inputs with queueing and timeout.
pub struct InputQueue {
    /// Queued inputs waiting to be delivered.
    queue: VecDeque<InputMessage>,
    /// Pending response channel when agent is waiting for input.
    pending_response: Option<oneshot::Sender<InputMessage>>,
    /// Maximum queue size (prevent memory issues).
    max_queue_size: usize,
    /// Default timeout for waiting for input.
    default_timeout: Duration,
    /// History of delivered inputs (for debugging/audit).
    history: Vec<InputMessage>,
    /// Maximum history size.
    max_history_size: usize,
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl InputQueue {
    /// Create a new input queue with default settings.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            pending_response: None,
            max_queue_size: 100,
            default_timeout: Duration::from_secs(300), // 5 minutes
            history: Vec::new(),
            max_history_size: 50,
        }
    }

    /// Create an input queue with custom configuration.
    pub fn with_config(max_queue_size: usize, default_timeout_secs: u64) -> Self {
        Self {
            queue: VecDeque::new(),
            pending_response: None,
            max_queue_size,
            default_timeout: Duration::from_secs(default_timeout_secs),
            history: Vec::new(),
            max_history_size: 50,
        }
    }

    /// Enqueue an input message.
    ///
    /// If there's a pending receiver waiting for input, delivers immediately.
    /// Otherwise, queues the message for later delivery.
    ///
    /// ## Returns
    /// - `Ok(true)` if message was delivered immediately
    /// - `Ok(false)` if message was queued
    /// - `Err` if queue is full
    pub fn enqueue(
        &mut self,
        content: String,
        sender_id: Option<Uuid>,
    ) -> Result<bool, InputQueueError> {
        let message = InputMessage::new(content, sender_id);

        // Try to deliver immediately if someone is waiting
        if let Some(sender) = self.pending_response.take() {
            if sender.send(message.clone()).is_ok() {
                self.add_to_history(message);
                debug!("Input delivered immediately");
                return Ok(true);
            }
            // Receiver dropped, queue the message instead
        }

        // Check queue capacity
        if self.queue.len() >= self.max_queue_size {
            return Err(InputQueueError::QueueFull {
                max_size: self.max_queue_size,
            });
        }

        self.queue.push_back(message);
        debug!("Input queued (queue size: {})", self.queue.len());
        Ok(false)
    }

    /// Wait for input with timeout.
    ///
    /// If there's a queued message, returns it immediately.
    /// Otherwise, waits for a message to arrive or times out.
    pub async fn wait_for_input(&mut self, timeout: Option<Duration>) -> Option<InputMessage> {
        // Check if there's already a queued message
        if let Some(message) = self.queue.pop_front() {
            self.add_to_history(message.clone());
            return Some(message);
        }

        // Create a channel to receive input
        let (tx, rx) = oneshot::channel();
        self.pending_response = Some(tx);

        let timeout = timeout.unwrap_or(self.default_timeout);

        // Wait with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(message)) => {
                debug!("Received input after waiting");
                Some(message)
            }
            Ok(Err(_)) => {
                debug!("Input channel closed");
                None
            }
            Err(_) => {
                debug!("Input wait timed out");
                self.pending_response = None;
                None
            }
        }
    }

    /// Get the number of queued inputs.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Check if there's a pending input request.
    pub fn is_waiting_for_input(&self) -> bool {
        self.pending_response.is_some()
    }

    /// Get a copy of queued messages (for inspection).
    pub fn peek_queue(&self) -> Vec<InputMessage> {
        self.queue.iter().cloned().collect()
    }

    /// Get input history.
    pub fn get_history(&self) -> &[InputMessage] {
        &self.history
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.pending_response = None;
    }

    /// Add message to history, trimming if needed.
    fn add_to_history(&mut self, message: InputMessage) {
        self.history.push(message);
        if self.history.len() > self.max_history_size {
            self.history.remove(0);
        }
    }
}

/// Errors that can occur with the input queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputQueueError {
    /// Queue has reached maximum capacity.
    QueueFull { max_size: usize },
    /// Input delivery failed.
    DeliveryFailed { reason: String },
}

impl std::fmt::Display for InputQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull { max_size } => {
                write!(f, "Input queue is full (max size: {})", max_size)
            }
            Self::DeliveryFailed { reason } => {
                write!(f, "Input delivery failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for InputQueueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_and_dequeue() {
        let mut queue = InputQueue::new();

        // Enqueue
        let result = queue.enqueue("test input".to_string(), None);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should be queued, not delivered

        assert_eq!(queue.queue_len(), 1);

        // Check peek
        let peeked = queue.peek_queue();
        assert_eq!(peeked.len(), 1);
        assert_eq!(peeked[0].content, "test input");
    }

    #[test]
    fn test_queue_full() {
        let mut queue = InputQueue::with_config(2, 300);

        queue.enqueue("input 1".to_string(), None).unwrap();
        queue.enqueue("input 2".to_string(), None).unwrap();

        let result = queue.enqueue("input 3".to_string(), None);
        assert!(matches!(
            result,
            Err(InputQueueError::QueueFull { max_size: 2 })
        ));
    }

    #[tokio::test]
    async fn test_immediate_pop() {
        let mut queue = InputQueue::new();
        queue.enqueue("queued input".to_string(), None).unwrap();

        let result = queue.wait_for_input(Some(Duration::from_millis(100))).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().content, "queued input");
        assert_eq!(queue.queue_len(), 0);
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut queue = InputQueue::new();

        let result = queue.wait_for_input(Some(Duration::from_millis(10))).await;
        assert!(result.is_none());
    }

    #[test]
    fn test_history() {
        let mut queue = InputQueue::with_config(10, 300);

        for i in 0..5 {
            queue.enqueue(format!("input {}", i), None).unwrap();
        }

        // Pop all to move to history
        let rt = tokio::runtime::Runtime::new().unwrap();
        for _ in 0..5 {
            rt.block_on(async {
                queue.wait_for_input(Some(Duration::from_millis(10))).await;
            });
        }

        assert_eq!(queue.get_history().len(), 5);
    }
}
