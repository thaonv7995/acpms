//! Thread-safe async log writer with buffering and auto-flush.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;

/// Thread-safe, async log writer
///
/// ## Features:
/// - Thread-safe via Arc<Mutex>
/// - Buffered writes for performance
/// - Auto-flush after each log line
/// - Cloneable for sharing across tasks
///
/// ## Usage:
/// ```ignore
/// let writer = LogWriter::new(stdout_pipe);
/// writer.log_raw("Hello from agent").await?;
/// ```
#[derive(Clone)]
pub struct LogWriter {
    writer: Arc<Mutex<BufWriter<Box<dyn AsyncWrite + Send + Unpin>>>>,
}

impl LogWriter {
    /// Create new log writer from any AsyncWrite
    pub fn new(writer: impl AsyncWrite + Send + Unpin + 'static) -> Self {
        Self {
            writer: Arc::new(Mutex::new(BufWriter::new(Box::new(writer)))),
        }
    }

    /// Write raw log line (adds newline and flushes)
    ///
    /// ## Important:
    /// - Automatically appends `\n`
    /// - Flushes immediately for visibility
    /// - Thread-safe (mutex lock)
    pub async fn log_raw(&self, raw: &str) -> Result<()> {
        let mut guard = self.writer.lock().await;

        guard
            .write_all(raw.as_bytes())
            .await
            .context("Failed to write log bytes")?;

        guard
            .write_all(b"\n")
            .await
            .context("Failed to write newline")?;

        // CRITICAL: Always flush to ensure immediate visibility
        guard.flush().await.context("Failed to flush log writer")?;

        Ok(())
    }

    /// Write multiple log lines at once (more efficient)
    pub async fn log_lines(&self, lines: &[&str]) -> Result<()> {
        let mut guard = self.writer.lock().await;

        for line in lines {
            guard
                .write_all(line.as_bytes())
                .await
                .context("Failed to write log bytes")?;
            guard
                .write_all(b"\n")
                .await
                .context("Failed to write newline")?;
        }

        // Flush once after all lines
        guard.flush().await.context("Failed to flush log writer")?;

        Ok(())
    }
}
