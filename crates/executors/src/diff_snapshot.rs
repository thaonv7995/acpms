use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
/// Diff snapshot structures for S3 storage
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trait for uploading diff snapshots to object storage (S3/MinIO).
///
/// Implemented by `StorageService` in `acpms-services`. This trait avoids
/// a cyclic dependency between executors and services crates.
#[async_trait::async_trait]
pub trait DiffStorageUploader: Send + Sync {
    /// Upload a JSON-serialized diff snapshot to the given S3 key.
    async fn upload_diff_snapshot(&self, key: &str, snapshot: &AttemptDiffSnapshot) -> Result<()>;

    /// Download raw object bytes from storage by key.
    async fn download_object_bytes(&self, key: &str) -> Result<Vec<u8>>;
}

/// Complete diff snapshot for an attempt, stored as JSON in S3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptDiffSnapshot {
    pub attempt_id: Uuid,
    pub task_id: Uuid,
    pub saved_at: DateTime<Utc>,
    pub base_branch: String,
    pub feature_branch: String,
    pub total_files: usize,
    pub total_additions: i32,
    pub total_deletions: i32,
    pub files: Vec<FileDiffData>,
    pub metadata: serde_json::Value,
}

/// Individual file diff data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiffData {
    pub change: String, // "added", "modified", "deleted", "renamed"
    pub path: String,
    pub old_path: Option<String>,
    pub additions: i32,
    pub deletions: i32,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

impl AttemptDiffSnapshot {
    /// Calculate total size in bytes of all file contents
    pub fn calculate_total_size(&self) -> i64 {
        self.files
            .iter()
            .map(|f| {
                let old_size = f.old_content.as_ref().map(|s| s.len()).unwrap_or(0);
                let new_size = f.new_content.as_ref().map(|s| s.len()).unwrap_or(0);
                (old_size + new_size) as i64
            })
            .sum()
    }

    /// Generate S3 key with date partitioning
    pub fn generate_s3_key(attempt_id: Uuid, timestamp: DateTime<Utc>) -> String {
        format!(
            "diffs/{}/{:02}/{:02}/{}.json",
            timestamp.year(),
            timestamp.month(),
            timestamp.day(),
            attempt_id
        )
    }
}
