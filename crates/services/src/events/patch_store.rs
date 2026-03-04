use super::PatchOperation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Sequenced patch for ordering guarantees
#[derive(Clone, Serialize, Deserialize)]
pub struct SequencedPatch {
    pub seq: u64,
    pub patch: PatchOperation,
    pub path: String,
    pub timestamp: i64,
}

/// Response from get_patches_since
pub enum PatchResponse {
    Patches(Vec<SequencedPatch>),
    GapDetected {
        oldest_available: u64,
        requested: u64,
        message: String,
    },
}

/// Patch store with history buffer and gap detection
pub struct PatchStore {
    patches: Arc<RwLock<HashMap<String, VecDeque<SequencedPatch>>>>,
    next_seq: Arc<RwLock<HashMap<String, AtomicU64>>>,
    oldest_seq: Arc<RwLock<HashMap<String, AtomicU64>>>,
    collection_seq: Arc<RwLock<HashMap<String, AtomicU64>>>,
    max_history: usize,
}

impl PatchStore {
    pub fn new(max_history: usize) -> Self {
        Self {
            patches: Arc::new(RwLock::new(HashMap::new())),
            next_seq: Arc::new(RwLock::new(HashMap::new())),
            oldest_seq: Arc::new(RwLock::new(HashMap::new())),
            collection_seq: Arc::new(RwLock::new(HashMap::new())),
            max_history,
        }
    }

    /// Push a new patch to the store
    pub async fn push_patch(&self, path: &str, operation: PatchOperation) {
        // Get next sequence for this path
        let seq = {
            let mut seq_map = self.next_seq.write().await;
            let atomic = seq_map
                .entry(path.to_string())
                .or_insert_with(|| AtomicU64::new(0));
            atomic.fetch_add(1, Ordering::SeqCst)
        };

        let sequenced = SequencedPatch {
            seq,
            patch: operation,
            path: path.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut patches = self.patches.write().await;
        let history = patches
            .entry(path.to_string())
            .or_insert_with(|| VecDeque::with_capacity(self.max_history));

        history.push_back(sequenced);

        // Maintain buffer size + track oldest
        if history.len() > self.max_history {
            if let Some(evicted) = history.pop_front() {
                let mut oldest_map = self.oldest_seq.write().await;
                let atomic = oldest_map
                    .entry(path.to_string())
                    .or_insert_with(|| AtomicU64::new(0));
                // `since` is treated as a cursor (next patch seq to fetch).
                // When evicting seq=N, the oldest available cursor becomes N+1.
                atomic.store(evicted.seq.saturating_add(1), Ordering::SeqCst);
            }
        }
    }

    /// Get patches since a specific sequence number
    /// Get patches since a cursor (next patch seq to fetch).
    pub async fn get_patches_since(&self, path: &str, requested_seq: u64) -> PatchResponse {
        // Check for gap
        let oldest = {
            let oldest_map = self.oldest_seq.read().await;
            oldest_map
                .get(path)
                .map(|a| a.load(Ordering::SeqCst))
                .unwrap_or(0)
        };

        if requested_seq > 0 && requested_seq < oldest {
            return PatchResponse::GapDetected {
                oldest_available: oldest,
                requested: requested_seq,
                message: format!(
                    "Sequence gap: requested {}, oldest {}",
                    requested_seq, oldest
                ),
            };
        }

        // Return patches
        let patches = self.patches.read().await;
        let result = patches
            .get(path)
            .map(|history| {
                history
                    .iter()
                    .filter(|p| p.seq >= requested_seq)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        PatchResponse::Patches(result)
    }

    /// Get latest sequence number for a path
    pub async fn get_latest_sequence(&self, path: &str) -> u64 {
        let seq_map = self.next_seq.read().await;
        seq_map
            .get(path)
            .map(|a| a.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Get the latest issued collection stream sequence ID for a key.
    pub async fn get_latest_collection_sequence(&self, key: &str) -> u64 {
        let seq_map = self.collection_seq.read().await;
        seq_map
            .get(key)
            .map(|a| a.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Reserve the next monotonic collection stream sequence ID.
    pub async fn reserve_collection_sequence(&self, key: &str) -> u64 {
        let mut seq_map = self.collection_seq.write().await;
        let atomic = seq_map
            .entry(key.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        atomic.fetch_add(1, Ordering::SeqCst).saturating_add(1)
    }

    /// Reserve a collection snapshot sequence ID and validate reconnect cursor.
    ///
    /// Returns `Err((requested_since_seq, max_available_sequence_id))` when the client cursor
    /// is ahead of the persisted sequence timeline.
    pub async fn reserve_collection_snapshot_sequence(
        &self,
        key: &str,
        since_seq: Option<u64>,
    ) -> Result<u64, (u64, u64)> {
        let max_available_sequence_id = self.get_latest_collection_sequence(key).await;

        if let Some(requested_since_seq) = since_seq {
            if requested_since_seq > max_available_sequence_id {
                return Err((requested_since_seq, max_available_sequence_id));
            }
        }

        Ok(self.reserve_collection_sequence(key).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_push_and_get_patches() {
        let store = PatchStore::new(10);
        let path = "/attempts/test";

        // Push 5 patches
        for i in 0..5 {
            store
                .push_patch(
                    path,
                    PatchOperation::Replace {
                        path: format!("/status"),
                        value: json!(i),
                    },
                )
                .await;
        }

        // Get all patches
        let response = store.get_patches_since(path, 0).await;
        match response {
            PatchResponse::Patches(patches) => {
                assert_eq!(patches.len(), 5);
            }
            _ => panic!("Expected patches"),
        }
    }

    #[tokio::test]
    async fn test_gap_detection() {
        let store = PatchStore::new(10);
        let path = "/attempts/test";

        // Push 20 patches (overflow buffer)
        for i in 0..20 {
            store
                .push_patch(
                    path,
                    PatchOperation::Replace {
                        path: "/value".into(),
                        value: json!(i),
                    },
                )
                .await;
        }

        // Request old sequence (should be evicted)
        let response = store.get_patches_since(path, 5).await;

        match response {
            PatchResponse::GapDetected {
                oldest_available,
                requested,
                ..
            } => {
                assert_eq!(requested, 5);
                // With max_history=10 and cursor semantics, the oldest available cursor should be >= 10.
                assert!(oldest_available >= 10);
            }
            _ => panic!("Expected gap detection"),
        }
    }

    #[tokio::test]
    async fn test_collection_sequence_reserve_is_monotonic() {
        let store = PatchStore::new(10);
        let key = "/collections/approvals/process-1";

        assert_eq!(store.get_latest_collection_sequence(key).await, 0);
        assert_eq!(store.reserve_collection_sequence(key).await, 1);
        assert_eq!(store.reserve_collection_sequence(key).await, 2);
        assert_eq!(store.get_latest_collection_sequence(key).await, 2);
    }

    #[tokio::test]
    async fn test_collection_snapshot_sequence_rejects_future_cursor() {
        let store = PatchStore::new(10);
        let key = "/collections/processes/attempt-1";

        store.reserve_collection_sequence(key).await;
        store.reserve_collection_sequence(key).await;

        let result = store
            .reserve_collection_snapshot_sequence(key, Some(10))
            .await;
        assert_eq!(result, Err((10, 2)));
    }

    #[tokio::test]
    async fn test_collection_snapshot_sequence_accepts_reconnect_cursor() {
        let store = PatchStore::new(10);
        let key = "/collections/processes/attempt-2";

        let first_snapshot = store
            .reserve_collection_snapshot_sequence(key, None)
            .await
            .expect("initial snapshot sequence");
        assert_eq!(first_snapshot, 1);

        store.reserve_collection_sequence(key).await; // sequence 2 (live patch)

        let reconnect_snapshot = store
            .reserve_collection_snapshot_sequence(key, Some(2))
            .await
            .expect("reconnect snapshot sequence");
        assert_eq!(reconnect_snapshot, 3);
    }
}
