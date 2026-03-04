//! In-memory message store for agent logs (vibe-kanban pattern)

use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt};

const HISTORY_BYTES: usize = 100_000 * 1024; // 100 MB buffer

/// Log message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum LogMsg {
    Stdout(String),
    Stderr(String),
    Finished,
}

impl LogMsg {
    pub fn approx_bytes(&self) -> usize {
        match self {
            LogMsg::Stdout(s) | LogMsg::Stderr(s) => s.len() + 16, // String + overhead
            LogMsg::Finished => 16,
        }
    }

    pub fn content(&self) -> Option<&str> {
        match self {
            LogMsg::Stdout(s) | LogMsg::Stderr(s) => Some(s),
            LogMsg::Finished => None,
        }
    }

    pub fn is_stdout(&self) -> bool {
        matches!(self, LogMsg::Stdout(_))
    }

    pub fn is_stderr(&self) -> bool {
        matches!(self, LogMsg::Stderr(_))
    }
}

struct StoredMsg {
    msg: LogMsg,
    bytes: usize,
}

struct Inner {
    history: VecDeque<StoredMsg>,
    total_bytes: usize,
}

/// Message store with circular buffer and broadcast
pub struct MsgStore {
    inner: RwLock<Inner>,
    sender: broadcast::Sender<LogMsg>,
}

impl MsgStore {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self {
            inner: RwLock::new(Inner {
                history: VecDeque::new(),
                total_bytes: 0,
            }),
            sender,
        }
    }

    /// Push message to store (circular buffer)
    pub fn push(&self, msg: LogMsg) {
        let _ = self.sender.send(msg.clone()); // Broadcast to live subscribers
        let bytes = msg.approx_bytes();

        let mut inner = self.inner.write().unwrap_or_else(|poisoned| {
            tracing::warn!("MsgStore write lock poisoned, recovering state");
            poisoned.into_inner()
        });
        // Maintain 100MB limit
        while inner.total_bytes.saturating_add(bytes) > HISTORY_BYTES {
            if let Some(front) = inner.history.pop_front() {
                inner.total_bytes = inner.total_bytes.saturating_sub(front.bytes);
            } else {
                break;
            }
        }
        inner.history.push_back(StoredMsg { msg, bytes });
        inner.total_bytes = inner.total_bytes.saturating_add(bytes);
    }

    /// Get history as vector
    pub fn history(&self) -> Vec<LogMsg> {
        let inner = self.inner.read().unwrap_or_else(|poisoned| {
            tracing::warn!("MsgStore read lock poisoned, recovering state");
            poisoned.into_inner()
        });
        inner.history.iter().map(|s| s.msg.clone()).collect()
    }

    /// Subscribe to live stream
    pub fn subscribe(&self) -> broadcast::Receiver<LogMsg> {
        self.sender.subscribe()
    }

    /// Get history + live stream combined
    pub fn history_plus_stream(&self) -> BoxStream<'static, Result<LogMsg, std::io::Error>> {
        let history = self.history();
        let mut rx = self.subscribe();

        let stream = async_stream::stream! {
            // Yield history first
            for msg in history {
                yield Ok(msg);
            }

            // Then live stream
            while let Ok(msg) = rx.recv().await {
                let is_finished = matches!(msg, LogMsg::Finished);
                yield Ok(msg);
                if is_finished {
                    break;
                }
            }
        };

        Box::pin(stream)
    }

    /// Spawn forwarder task (consumes stream and pushes to store)
    pub fn spawn_forwarder<S>(self: Arc<Self>, mut stream: S) -> tokio::task::JoinHandle<()>
    where
        S: Stream<Item = Result<LogMsg, std::io::Error>> + Send + Unpin + 'static,
    {
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                let is_finished = matches!(msg, LogMsg::Finished);
                self.push(msg);
                if is_finished {
                    break;
                }
            }
        })
    }
}

impl Default for MsgStore {
    fn default() -> Self {
        Self::new()
    }
}
