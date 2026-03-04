pub mod patch_builder;
pub mod patch_store;
pub mod streams;

pub use patch_builder::PatchBuilder;
pub use patch_store::{PatchResponse, PatchStore, SequencedPatch};
pub use streams::StreamService;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Log message types for broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogMsg {
    // Existing
    Stdout(String),
    Stderr(String),
    Finished,

    // New: JSON Patch
    JsonPatch {
        path: String,
        operation: PatchOperation,
        sequence: u64,
        timestamp: DateTime<Utc>,
    },

    // New: Initial snapshot
    Snapshot {
        path: String,
        data: Value,
        sequence: u64,
    },
}

/// JSON Patch operations (RFC 6902)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum PatchOperation {
    #[serde(rename = "add")]
    Add { path: String, value: Value },

    #[serde(rename = "replace")]
    Replace { path: String, value: Value },

    #[serde(rename = "remove")]
    Remove { path: String },

    #[serde(rename = "move")]
    Move { from: String, path: String },

    #[serde(rename = "copy")]
    Copy { from: String, path: String },

    #[serde(rename = "test")]
    Test { path: String, value: Value },
}

/// Stream message sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamMessage {
    Snapshot {
        seq: u64,
        path: String,
        data: Value,
    },
    Patch {
        seq: u64,
        path: String,
        operation: PatchOperation,
    },
    GapDetected {
        oldest_available: u64,
        requested: u64,
    },
}
