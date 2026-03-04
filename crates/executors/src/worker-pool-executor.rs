use crate::{AgentJob, ExecutorOrchestrator, WorkerPoolConfig};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Job execution logic with retry and timeout
pub struct JobExecutor;

impl JobExecutor {
    /// Process a job with retry logic using project settings.
    ///
    /// This method uses the job's embedded settings (timeout, max_retries, auto_retry)
    /// which are populated from project settings when the job is created.
    pub async fn process_with_retry(
        job: AgentJob,
        orchestrator: Arc<ExecutorOrchestrator>,
        cancel_channels: Arc<Mutex<HashMap<Uuid, watch::Sender<bool>>>>,
        config: &WorkerPoolConfig,
    ) -> Result<()> {
        // Use job's timeout from project settings, fallback to config default
        let timeout = if job.timeout_secs > 0 {
            Duration::from_secs(job.timeout_secs)
        } else {
            config.job_timeout
        };

        // Create cancellation channel for this attempt
        let (cancel_tx, cancel_rx) = watch::channel(false);
        cancel_channels
            .lock()
            .await
            .insert(job.attempt_id, cancel_tx.clone());

        info!(
            "Executing job for attempt {} (timeout: {:?}, retry: {}/{})",
            job.attempt_id, timeout, job.retry_count, job.max_retries
        );

        let result =
            Self::execute_with_timeout(&job, orchestrator.clone(), cancel_rx, timeout).await;

        // Cleanup cancellation channel
        cancel_channels.lock().await.remove(&job.attempt_id);

        match result {
            Ok(_) => {
                info!("Job for attempt {} completed successfully", job.attempt_id);
                Ok(())
            }
            Err(e) => {
                // Check if this is a cancellation
                if e.to_string().to_lowercase().contains("cancelled") {
                    info!("Job for attempt {} was cancelled", job.attempt_id);
                    return Err(e);
                }

                // Log the failure
                error!(
                    "Job for attempt {} failed (retry {}/{}): {}",
                    job.attempt_id, job.retry_count, job.max_retries, e
                );

                // Note: Auto-retry scheduling is handled by the orchestrator's handle_failure method
                // which has access to the database pool for creating new attempts
                Err(e)
            }
        }
    }

    /// Execute job with timeout enforcement using job's embedded timeout.
    async fn execute_with_timeout(
        job: &AgentJob,
        orchestrator: Arc<ExecutorOrchestrator>,
        cancel_rx: watch::Receiver<bool>,
        timeout: Duration,
    ) -> Result<()> {
        // Execute with timeout and cancellation support
        tokio::select! {
            result = orchestrator.execute_task_with_cancel_review(
                job.attempt_id,
                job.task_id,
                job.repo_path.clone(),
                job.instruction.clone(),
                cancel_rx,
                job.require_review,
            ) => {
                result.context("Task execution failed")
            }
            _ = tokio::time::sleep(timeout) => {
                warn!("Job for attempt {} timed out after {:?}", job.attempt_id, timeout);
                anyhow::bail!("Job execution timeout after {:?}", timeout)
            }
        }
    }

    /// Determine if an error is retriable.
    ///
    /// Uses the same logic as RetryHandler::is_retriable_error for consistency.
    pub fn is_retriable(error: &anyhow::Error) -> bool {
        let error_msg = error.to_string().to_lowercase();

        // Non-retriable errors (same as RetryHandler)
        let non_retriable = [
            "permission denied",
            "authentication failed",
            "invalid credentials",
            "invalid token",
            "unauthorized",
            "forbidden",
            "not found",
            "cancelled",
            "rate limit",
        ];

        if non_retriable.iter().any(|e| error_msg.contains(e)) {
            return false;
        }

        // Retriable errors
        error_msg.contains("network")
            || error_msg.contains("timeout")
            || error_msg.contains("connection")
            || error_msg.contains("temporary")
            || error_msg.contains("worktree cleanup")
            || error_msg.contains("internal")
            || error_msg.contains("unavailable")
    }
}

/// Enhanced cancellation handler with graceful and force kill support.
pub struct CancellationHandler;

impl CancellationHandler {
    /// Cancel a job with optional reason.
    ///
    /// ## Arguments
    /// - `cancel_channels`: Map of attempt_id to cancel senders
    /// - `attempt_id`: The attempt to cancel
    /// - `reason`: Optional cancellation reason
    /// - `force`: If true, force kill after graceful timeout
    ///
    /// ## Returns
    /// - `Ok(true)` if cancellation signal was sent
    /// - `Ok(false)` if no active job found
    /// - `Err` if cancellation failed
    pub async fn cancel(
        cancel_channels: &Arc<Mutex<HashMap<Uuid, watch::Sender<bool>>>>,
        attempt_id: Uuid,
        reason: Option<String>,
        _force: bool,
    ) -> Result<bool> {
        let channels = cancel_channels.lock().await;

        if let Some(tx) = channels.get(&attempt_id) {
            let reason_str = reason.unwrap_or_else(|| "User requested cancellation".to_string());
            info!("Cancelling job for attempt {}: {}", attempt_id, reason_str);

            tx.send(true)
                .context("Failed to send cancellation signal")?;

            // Note: Force kill is handled by the orchestrator's terminate_session method
            // which is called when the cancel signal is received

            Ok(true)
        } else {
            debug!("No active job found for attempt {}", attempt_id);
            Ok(false)
        }
    }

    /// Check if a job is currently running.
    pub async fn is_running(
        cancel_channels: &Arc<Mutex<HashMap<Uuid, watch::Sender<bool>>>>,
        attempt_id: Uuid,
    ) -> bool {
        cancel_channels.lock().await.contains_key(&attempt_id)
    }
}
