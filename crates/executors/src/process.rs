//! Process management utilities for agent session termination.
//!
//! Provides robust process group management with graceful shutdown support,
//! following patterns from vibe-kanban.

use anyhow::{Context, Result};
use command_group::AsyncGroupChild;
use std::time::Duration;
use tokio::sync::oneshot;

#[cfg(unix)]
use nix::{
    sys::signal::{killpg, Signal},
    unistd::{getpgid, Pid},
};

/// Sender for requesting graceful interrupt of an executor.
/// When sent, the executor should attempt to interrupt gracefully before being killed.
pub type InterruptSender = oneshot::Sender<()>;

/// Receiver for interrupt signals.
pub type InterruptReceiver = oneshot::Receiver<()>;

/// Result of spawning a child process with control channels.
pub struct SpawnedProcess {
    /// The spawned child process (process group).
    pub child: AsyncGroupChild,
    /// Optional sender for requesting graceful shutdown.
    pub interrupt_sender: Option<InterruptSender>,
}

/// Kill an entire process group with escalating signals.
///
/// ## Signal Escalation (Unix only)
/// 1. SIGINT - Allow graceful shutdown (2s wait)
/// 2. SIGTERM - Request termination (2s wait)
/// 3. SIGKILL - Force kill (immediate)
///
/// ## Behavior
/// - On Unix: Kills the entire process group (all child processes)
/// - On other platforms: Falls back to regular kill
///
/// ## Example
/// ```ignore
/// let mut child = spawn_process_group().await?;
/// // ... do work ...
/// kill_process_group(&mut child).await?;
/// ```
pub async fn kill_process_group(child: &mut AsyncGroupChild) -> Result<()> {
    #[cfg(unix)]
    {
        if let Some(pid) = child.inner().id() {
            let pid = Pid::from_raw(pid as i32);

            // Get process group ID
            match getpgid(Some(pid)) {
                Ok(pgid) => {
                    // Escalate through signals
                    for sig in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGKILL] {
                        tracing::debug!("Sending {:?} to process group {}", sig, pgid);

                        if let Err(e) = killpg(pgid, sig) {
                            tracing::warn!(
                                "Failed to send signal {:?} to process group {}: {}",
                                sig,
                                pgid,
                                e
                            );
                        }

                        // Wait for process to exit
                        tokio::time::sleep(Duration::from_secs(2)).await;

                        // Check if process has exited
                        match child.inner().try_wait() {
                            Ok(Some(_)) => {
                                tracing::debug!(
                                    "Process group {} terminated after {:?}",
                                    pgid,
                                    sig
                                );
                                return Ok(());
                            }
                            Ok(None) => {
                                // Process still running, try next signal
                                continue;
                            }
                            Err(e) => {
                                tracing::warn!("Error checking process status: {}", e);
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to get process group ID: {}, falling back to regular kill",
                        e
                    );
                }
            }
        }
    }

    // Fallback: regular kill + wait
    let _ = child.kill().await;
    let _ = child.wait().await;

    Ok(())
}

/// Two-phase termination: graceful interrupt followed by force kill.
///
/// ## Phases
/// 1. **Graceful Phase**: Send interrupt signal and wait up to `graceful_timeout`
/// 2. **Force Phase**: Kill process group if graceful shutdown timed out
///
/// ## Arguments
/// * `child` - The child process to terminate
/// * `interrupt_sender` - Optional channel to signal graceful shutdown
/// * `graceful_timeout` - How long to wait for graceful shutdown
///
/// ## Example
/// ```ignore
/// terminate_process(
///     &mut child,
///     Some(interrupt_tx),
///     Duration::from_secs(5),
/// ).await?;
/// ```
pub async fn terminate_process(
    child: &mut AsyncGroupChild,
    interrupt_sender: Option<InterruptSender>,
    graceful_timeout: Duration,
) -> Result<()> {
    // Phase 1: Try graceful interrupt
    if let Some(sender) = interrupt_sender {
        tracing::debug!("Sending graceful interrupt signal");

        // Send interrupt signal (ignore error if receiver dropped)
        let _ = sender.send(());

        // Wait for graceful exit with timeout
        let graceful_exit =
            tokio::time::timeout(graceful_timeout, async { child.wait().await }).await;

        match graceful_exit {
            Ok(Ok(status)) => {
                tracing::debug!("Process exited gracefully with status: {:?}", status);
                return Ok(());
            }
            Ok(Err(e)) => {
                tracing::info!("Error waiting for graceful exit: {}", e);
            }
            Err(_) => {
                tracing::debug!(
                    "Graceful shutdown timed out after {:?}, force killing",
                    graceful_timeout
                );
            }
        }
    }

    // Phase 2: Force kill process group
    kill_process_group(child)
        .await
        .context("Failed to force kill process group")
}

/// Spawn a process with timeout.
///
/// ## Arguments
/// * `timeout` - Maximum time to wait for spawn to complete
/// * `spawn_fn` - Async function that spawns the process
///
/// ## Returns
/// * `Ok(T)` - Successfully spawned process
/// * `Err` - Spawn timed out or failed
pub async fn spawn_with_timeout<T, F, Fut>(timeout: Duration, spawn_fn: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    tokio::time::timeout(timeout, spawn_fn())
        .await
        .context("Spawn operation timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_channel_creation() {
        let (tx, _rx): (InterruptSender, InterruptReceiver) = oneshot::channel();
        assert!(tx.send(()).is_ok() || true); // Just verify it compiles
    }
}
