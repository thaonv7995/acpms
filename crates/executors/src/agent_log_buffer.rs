//! JSONL-only log storage (Vibe Kanban-style).
//! All logs append to local JSONL file; upload to S3 on attempt completion.
//! No agent_logs DB - JSONL is the single source of truth.

use acpms_db::models::AgentLog;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

const FLUSH_INTERVAL_MS: u64 = 100;
const MAX_BUFFER_SIZE: usize = 50;

fn log_dir() -> PathBuf {
    std::env::var("ACPMS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("acpms-logs"))
}

#[derive(Clone)]
struct LogEntry {
    id: Uuid,
    attempt_id: Uuid,
    log_type: String,
    content: String,
    created_at: DateTime<Utc>,
}

struct BufferInner {
    buffer: Vec<LogEntry>,
}

pub struct AgentLogBuffer {
    #[allow(dead_code)] // Kept for init signature; JSONL-only, no DB
    pool: PgPool,
    inner: Mutex<BufferInner>,
}

static LOG_BUFFER: OnceCell<Arc<AgentLogBuffer>> = OnceCell::new();

impl AgentLogBuffer {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            inner: Mutex::new(BufferInner {
                buffer: Vec::with_capacity(MAX_BUFFER_SIZE),
            }),
        }
    }

    /// Push a log entry to buffer. Returns (id, created_at) for immediate broadcast.
    /// Does not block on DB.
    pub async fn push(
        &self,
        attempt_id: Uuid,
        log_type: &str,
        content: &str,
    ) -> (Uuid, DateTime<Utc>) {
        let id = Uuid::new_v4();
        let created_at = Utc::now();

        let mut inner = self.inner.lock().await;
        inner.buffer.push(LogEntry {
            id,
            attempt_id,
            log_type: log_type.to_string(),
            content: content.to_string(),
            created_at,
        });

        if inner.buffer.len() >= MAX_BUFFER_SIZE {
            drop(inner);
            self.flush().await.ok();
        }

        (id, created_at)
    }

    /// Flush buffer to JSONL. Called periodically and on buffer full.
    pub async fn flush(&self) -> Result<usize> {
        let entries = {
            let mut inner = self.inner.lock().await;
            if inner.buffer.is_empty() {
                return Ok(0);
            }
            std::mem::take(&mut inner.buffer)
        };

        let count = entries.len();
        if count == 0 {
            return Ok(0);
        }

        // Append to local JSONL files for S3 upload on attempt completion (Vibe Kanban style)
        let by_attempt: HashMap<Uuid, Vec<&LogEntry>> =
            entries.iter().fold(HashMap::new(), |mut m, e| {
                m.entry(e.attempt_id).or_default().push(e);
                m
            });
        for (attempt_id, attempt_entries) in by_attempt {
            if let Err(e) = append_to_jsonl_file(attempt_id, attempt_entries).await {
                tracing::warn!(
                    "Failed to append to JSONL file for attempt {}: {}",
                    attempt_id,
                    e
                );
            }
        }

        Ok(count)
    }

    /// Flush remaining buffer. Call when attempt completes to ensure persistence.
    pub async fn flush_all(&self) -> Result<usize> {
        self.flush().await
    }
}

/// Initialize the global log buffer. Call once at server startup.
/// Idempotent: subsequent calls are no-ops.
pub fn init_agent_log_buffer(pool: PgPool) {
    if LOG_BUFFER.get().is_some() {
        return;
    }
    let buffer = Arc::new(AgentLogBuffer::new(pool));
    if LOG_BUFFER.set(buffer.clone()).is_err() {
        return; // Already initialized by another thread
    }

    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(FLUSH_INTERVAL_MS));
        loop {
            interval.tick().await;
            if let Some(b) = LOG_BUFFER.get() {
                if let Err(e) = b.flush().await {
                    tracing::warn!("Agent log buffer flush error: {}", e);
                }
            }
        }
    });
}

/// Get the global buffer. Panics if not initialized.
fn get_buffer() -> &'static Arc<AgentLogBuffer> {
    LOG_BUFFER.get().expect("Agent log buffer not initialized")
}

/// Push to buffer and return (id, created_at) for broadcast.
pub async fn buffer_agent_log(
    attempt_id: Uuid,
    log_type: &str,
    content: &str,
) -> (Uuid, DateTime<Utc>) {
    get_buffer().push(attempt_id, log_type, content).await
}

/// Flush buffer (e.g. when attempt completes).
pub async fn flush_agent_log_buffer() -> Result<usize> {
    get_buffer().flush_all().await
}

/// Path to the JSONL log file for an attempt.
pub fn get_attempt_log_file_path(attempt_id: Uuid) -> PathBuf {
    log_dir().join(format!("{}.jsonl", attempt_id))
}

/// Parse JSONL tail bytes to AgentLogs, returning at most `max_entries` most recent.
/// First line may be partial (from tail read); it is skipped.
/// Caps parse work when bytes are from a tail read.
pub fn parse_jsonl_tail_to_agent_logs(bytes: &[u8], max_entries: usize) -> Vec<AgentLog> {
    let mut logs = parse_jsonl_to_agent_logs(bytes);
    logs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    logs.truncate(max_entries);
    logs
}

/// Parse JSONL bytes to AgentLogs. Deduplicates by id (last wins for updates).
pub fn parse_jsonl_to_agent_logs(bytes: &[u8]) -> Vec<AgentLog> {
    let mut by_id: std::collections::HashMap<Uuid, AgentLog> = std::collections::HashMap::new();
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let v: serde_json::Value = match serde_json::from_slice(line) {
            Ok(x) => x,
            _ => continue,
        };
        let id = match v
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(x) => x,
            None => continue,
        };
        let attempt_id = match v
            .get("attempt_id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(x) => x,
            None => continue,
        };
        let log_type = match v.get("log_type").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let content = match v.get("content").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let created_at = match v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        {
            Some(dt) => dt.with_timezone(&Utc),
            None => continue,
        };
        // For duplicates (updates): keep original created_at for sort order, use new content
        let entry = by_id.entry(id).or_insert_with(|| AgentLog {
            id,
            attempt_id,
            log_type: log_type.clone(),
            content: content.clone(),
            created_at,
        });
        entry.content = content;
        entry.log_type = log_type;
    }
    let mut logs: Vec<AgentLog> = by_id.into_values().collect();
    logs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    logs
}

/// Read JSONL log file content for upload. Returns empty vec if file does not exist.
pub async fn read_attempt_log_file(attempt_id: Uuid) -> Result<Vec<u8>> {
    let path = get_attempt_log_file_path(attempt_id);
    match tokio::fs::read(&path).await {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Read first `max_bytes` of JSONL log file (head). Caps I/O for pagination with "before" cursor.
pub async fn read_attempt_log_file_head(attempt_id: Uuid, max_bytes: usize) -> Result<Vec<u8>> {
    let path = get_attempt_log_file_path(attempt_id);
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut buf = vec![0u8; max_bytes];
    let n = tokio::io::AsyncReadExt::read(&mut file, &mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

/// Read last `max_bytes` of JSONL log file (tail). Caps I/O for large files.
/// Returns empty vec if file does not exist or is empty.
pub async fn read_attempt_log_file_tail(attempt_id: Uuid, max_bytes: usize) -> Result<Vec<u8>> {
    use tokio::io::AsyncSeekExt;
    let path = get_attempt_log_file_path(attempt_id);
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata().await?;
    let len = meta.len() as usize;
    if len == 0 {
        return Ok(Vec::new());
    }
    let to_read = len.min(max_bytes);
    let start = len.saturating_sub(to_read);
    file.seek(std::io::SeekFrom::Start(start as u64)).await?;
    let mut buf = vec![0u8; to_read];
    let n = tokio::io::AsyncReadExt::read(&mut file, &mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

/// Append a single log entry to JSONL (for normalized entries from orchestrator-status).
/// Used when insert_agent_log would have been called - JSONL-only, no DB.
pub async fn append_log_to_jsonl(
    attempt_id: Uuid,
    log_type: &str,
    content: &str,
    id: Uuid,
    created_at: DateTime<Utc>,
) -> Result<()> {
    let entry = LogEntry {
        id,
        attempt_id,
        log_type: log_type.to_string(),
        content: content.to_string(),
        created_at,
    };
    append_to_jsonl_file(attempt_id, vec![&entry]).await
}

async fn append_to_jsonl_file(attempt_id: Uuid, entries: Vec<&LogEntry>) -> Result<()> {
    let dir = log_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .context("Failed to create log dir")?;

    let path = dir.join(format!("{}.jsonl", attempt_id));
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .context("Failed to open log file for append")?;

    for entry in entries {
        let line = serde_json::json!({
            "id": entry.id,
            "attempt_id": entry.attempt_id,
            "log_type": entry.log_type,
            "content": entry.content,
            "created_at": entry.created_at,
        });
        let s = serde_json::to_string(&line).context("Failed to serialize log entry")?;
        tokio::io::AsyncWriteExt::write_all(&mut file, s.as_bytes()).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, b"\n").await?;
    }

    Ok(())
}
