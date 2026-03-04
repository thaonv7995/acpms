use super::{PatchOperation, PatchResponse, PatchStore, StreamMessage};
use crate::StorageService;
use acpms_db::models::TaskAttempt;
use acpms_executors::{read_attempt_log_file, AgentEvent};
use futures::stream::{Stream, StreamExt as FuturesStreamExt};
use sqlx::PgPool;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

const SNAPSHOT_LOG_LIMIT: i64 = 50;

/// Service for streaming task attempts with JSON Patch
pub struct StreamService {
    patch_store: Arc<PatchStore>,
    broadcast_tx: broadcast::Sender<AgentEvent>,
    db_pool: PgPool,
    storage_service: Option<Arc<StorageService>>,
}

impl StreamService {
    pub fn new(
        patch_store: Arc<PatchStore>,
        broadcast_tx: broadcast::Sender<AgentEvent>,
        db_pool: PgPool,
    ) -> Self {
        Self {
            patch_store,
            broadcast_tx,
            db_pool,
            storage_service: None,
        }
    }

    pub fn with_storage(mut self, storage: Arc<StorageService>) -> Self {
        self.storage_service = Some(storage);
        self
    }

    /// Stream task attempt with catch-up support
    pub async fn stream_task_attempt_with_catchup(
        &self,
        attempt_id: Uuid,
        since_seq: Option<u64>,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamMessage, std::convert::Infallible>> + Send>> {
        let path = format!("/attempts/{}", attempt_id);
        let since = since_seq.unwrap_or(0);

        // Check for gap
        let patch_response = self.patch_store.get_patches_since(&path, since).await;

        let live_stream = self.subscribe_live_patches(path.clone());

        type RetStream =
            Pin<Box<dyn Stream<Item = Result<StreamMessage, std::convert::Infallible>> + Send>>;

        match patch_response {
            PatchResponse::GapDetected {
                oldest_available,
                requested,
                ..
            } => {
                // Send gap signal + new snapshot
                let snapshot = self.generate_snapshot(&path, attempt_id).await;
                let gap_msg = StreamMessage::GapDetected {
                    oldest_available,
                    requested,
                };

                Box::pin(FuturesStreamExt::chain(
                    futures::stream::iter(vec![
                        Result::<StreamMessage, std::convert::Infallible>::Ok(gap_msg),
                        Result::<StreamMessage, std::convert::Infallible>::Ok(
                            StreamMessage::Snapshot {
                                seq: snapshot.0,
                                path: snapshot.1,
                                data: snapshot.2,
                            },
                        ),
                    ]),
                    live_stream,
                )) as RetStream
            }

            PatchResponse::Patches(patches) => {
                if since == 0 {
                    // New client - send snapshot + live
                    let snapshot = self.generate_snapshot(&path, attempt_id).await;
                    Box::pin(FuturesStreamExt::chain(
                        futures::stream::iter(vec![Result::<
                            StreamMessage,
                            std::convert::Infallible,
                        >::Ok(
                            StreamMessage::Snapshot {
                                seq: snapshot.0,
                                path: snapshot.1,
                                data: snapshot.2,
                            },
                        )]),
                        live_stream,
                    )) as RetStream
                } else {
                    // Reconnecting client - send missed patches + live
                    let catch_up = futures::stream::iter(patches.into_iter().map(|p| {
                        Result::<StreamMessage, std::convert::Infallible>::Ok(
                            StreamMessage::Patch {
                                // `seq` is treated as a cursor (next patch seq to fetch).
                                seq: p.seq.saturating_add(1),
                                path: p.path,
                                operation: p.patch,
                            },
                        )
                    }));

                    Box::pin(FuturesStreamExt::chain(catch_up, live_stream)) as RetStream
                }
            }
        }
    }

    /// Generate snapshot of current state
    async fn generate_snapshot(
        &self,
        path: &str,
        attempt_id: Uuid,
    ) -> (u64, String, serde_json::Value) {
        // Fetch attempt from DB (includes s3_log_key)
        let attempt = sqlx::query_as::<_, TaskAttempt>("SELECT * FROM task_attempts WHERE id = $1")
            .bind(attempt_id)
            .fetch_optional(&self.db_pool)
            .await
            .ok()
            .flatten();

        let (recent_rows, has_more_logs) =
            if let (Some(ref storage), Some(ref att)) = (&self.storage_service, &attempt) {
                if let Some(ref s3_key) = att.s3_log_key {
                    // Load from S3 JSONL
                    match storage.get_log_bytes(s3_key).await {
                        Ok(bytes) => {
                            let all_logs: Vec<serde_json::Value> = bytes
                                .split(|&b| b == b'\n')
                                .filter(|line| !line.is_empty())
                                .filter_map(|line| serde_json::from_slice(line).ok())
                                .collect();
                            let total = all_logs.len() as i64;
                            let has_more = total > SNAPSHOT_LOG_LIMIT;
                            let start = if has_more {
                                (total - SNAPSHOT_LOG_LIMIT) as usize
                            } else {
                                0
                            };
                            let recent: Vec<serde_json::Value> = all_logs[start..]
                                .iter()
                                .map(|row| {
                                    let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                    let created_at = row
                                        .get("created_at")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);
                                    serde_json::json!({
                                        "id": id,
                                        "attempt_id": row.get("attempt_id"),
                                        "log_type": row.get("log_type"),
                                        "content": row.get("content"),
                                        "created_at": created_at,
                                        "timestamp": created_at,
                                        "tool_name": serde_json::Value::Null
                                    })
                                })
                                .collect();
                            (recent, has_more)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load logs from S3 for attempt {}: {}",
                                attempt_id,
                                e
                            );
                            (vec![], false)
                        }
                    }
                } else {
                    (vec![], false)
                }
            } else {
                (vec![], false)
            };

        let (recent_rows, has_more_logs) = if recent_rows.is_empty() {
            // Fallback: load from local JSONL (no agent_logs - Vibe Kanban style)
            match read_attempt_log_file(attempt_id).await {
                Ok(bytes) if !bytes.is_empty() => {
                    let all_logs: Vec<serde_json::Value> = bytes
                        .split(|&b| b == b'\n')
                        .filter(|line| !line.is_empty())
                        .filter_map(|line| {
                            let v: serde_json::Value = serde_json::from_slice(line).ok()?;
                            let id = v.get("id")?.as_str()?;
                            let created_at = v.get("created_at")?.clone();
                            Some(serde_json::json!({
                                "id": id,
                                "attempt_id": v.get("attempt_id"),
                                "log_type": v.get("log_type"),
                                "content": v.get("content"),
                                "created_at": created_at,
                                "timestamp": created_at,
                                "tool_name": serde_json::Value::Null
                            }))
                        })
                        .collect();
                    let total = all_logs.len() as i64;
                    let has_more = total > SNAPSHOT_LOG_LIMIT;
                    let start = if has_more {
                        (total - SNAPSHOT_LOG_LIMIT) as usize
                    } else {
                        0
                    };
                    let recent: Vec<serde_json::Value> = all_logs.into_iter().skip(start).collect();
                    (recent, has_more)
                }
                _ => (vec![], false),
            }
        } else {
            (recent_rows, has_more_logs)
        };

        let seq = self.patch_store.get_latest_sequence(path).await;

        let data = serde_json::json!({
            "attempt": attempt,
            "logs": recent_rows,
            "has_more_logs": has_more_logs,
            "snapshot_limit": SNAPSHOT_LOG_LIMIT,
        });

        (seq, path.to_string(), data)
    }

    /// Subscribe to live patches for a path, pushing each to PatchStore for sequence tracking
    fn subscribe_live_patches(
        &self,
        _path: String,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamMessage, std::convert::Infallible>> + Send>> {
        let rx = self.broadcast_tx.subscribe();
        let stream = BroadcastStream::new(rx);
        let patch_store = self.patch_store.clone();

        Box::pin(FuturesStreamExt::filter_map(stream, move |msg_result| {
            let patch_store = patch_store.clone();
            async move {
                match msg_result {
                    Ok(AgentEvent::Log(log)) => {
                        let path = format!("/attempts/{}", log.attempt_id);
                        let operation = PatchOperation::Add {
                            path: "/logs/-".into(),
                            value: serde_json::json!({
                                "id": log.id,
                                "attempt_id": log.attempt_id,
                                "log_type": log.log_type,
                                "content": log.content,
                                "timestamp": log.timestamp,
                                "created_at": log.created_at,
                                "tool_name": log.tool_name,
                            }),
                        };

                        // Push to PatchStore for sequence tracking
                        patch_store.push_patch(&path, operation.clone()).await;
                        let seq = patch_store.get_latest_sequence(&path).await;

                        Some(Ok(StreamMessage::Patch {
                            seq,
                            path,
                            operation,
                        }))
                    }
                    Ok(AgentEvent::Status(status)) => {
                        let path = format!("/attempts/{}", status.attempt_id);
                        let status_value = match serde_json::to_value(status.status) {
                            Ok(value) => value,
                            Err(err) => {
                                tracing::warn!(
                                    attempt_id = %status.attempt_id,
                                    error = %err,
                                    "Failed to serialize attempt status for stream patch"
                                );
                                serde_json::Value::Null
                            }
                        };
                        let operation = PatchOperation::Replace {
                            path: "/status".into(),
                            value: status_value,
                        };

                        // Push to PatchStore for sequence tracking
                        patch_store.push_patch(&path, operation.clone()).await;
                        let seq = patch_store.get_latest_sequence(&path).await;

                        Some(Ok(StreamMessage::Patch {
                            seq,
                            path,
                            operation,
                        }))
                    }
                    _ => None,
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_patch_store_sequence_tracking() {
        let patch_store = Arc::new(PatchStore::new(100));
        let path = "/attempts/test-123";

        // Push 3 patches (they get seq 0, 1, 2)
        for i in 0..3 {
            let operation = PatchOperation::Add {
                path: "/logs/-".into(),
                value: json!({ "log_id": i }),
            };
            patch_store.push_patch(path, operation).await;
        }

        // Verify next sequence number (3 after 3 pushes)
        let seq = patch_store.get_latest_sequence(path).await;
        assert_eq!(seq, 3, "Expected next sequence 3 after 3 pushes");

        // Get patches since cursor 0 (should get seq 0, 1 and 2)
        let response = patch_store.get_patches_since(path, 0).await;
        match response {
            PatchResponse::Patches(patches) => {
                assert_eq!(patches.len(), 3, "Expected 3 patches from cursor 0");
                assert_eq!(patches[0].seq, 0);
                assert_eq!(patches[1].seq, 1);
                assert_eq!(patches[2].seq, 2);
            }
            PatchResponse::GapDetected { .. } => {
                panic!("Unexpected gap detection");
            }
        }
    }

    #[tokio::test]
    async fn test_patch_store_gap_detection() {
        // Use small buffer to trigger gap detection
        let patch_store = Arc::new(PatchStore::new(5));
        let path = "/attempts/gap-test";

        // Push 10 patches (overflow buffer, seq 0-9)
        for i in 0..10 {
            let operation = PatchOperation::Replace {
                path: "/status".into(),
                value: json!(format!("status_{}", i)),
            };
            patch_store.push_patch(path, operation).await;
        }

        // Request old cursor that was evicted (seq 2 was pushed out)
        let response = patch_store.get_patches_since(path, 2).await;
        match response {
            PatchResponse::GapDetected {
                oldest_available,
                requested,
                ..
            } => {
                assert_eq!(requested, 2);
                // After 10 pushes with buffer size 5, oldest cursor is 5 (0-4 evicted).
                assert!(
                    oldest_available >= 5,
                    "Oldest cursor should be >= 5 after eviction"
                );
            }
            PatchResponse::Patches(_) => {
                panic!("Expected gap detection");
            }
        }
    }

    #[tokio::test]
    async fn test_patch_store_new_client() {
        let patch_store = Arc::new(PatchStore::new(100));
        let path = "/attempts/new-client";

        // Push some patches (seq 0, 1, 2, 3, 4)
        for i in 0..5 {
            let operation = PatchOperation::Add {
                path: "/logs/-".into(),
                value: json!({ "message": format!("log {}", i) }),
            };
            patch_store.push_patch(path, operation).await;
        }

        // Cursor semantics: since=0 means "give me everything you have".
        let response = patch_store.get_patches_since(path, 0).await;
        match response {
            PatchResponse::Patches(patches) => {
                assert_eq!(
                    patches.len(),
                    5,
                    "New client with since=0 should get all patches (seq 0-4)"
                );
            }
            PatchResponse::GapDetected { .. } => {
                panic!("New client should not trigger gap detection");
            }
        }
    }
}
