use crate::{AgentJob, ExecutorOrchestrator, JobExecutor, JobPriority, WorkerPoolConfig};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use uuid::Uuid;

/// Worker pool manages concurrent task execution with queue, retry, and cancellation
pub struct WorkerPool {
    high_queue: Arc<deadqueue::unlimited::Queue<AgentJob>>,
    normal_queue: Arc<deadqueue::unlimited::Queue<AgentJob>>,
    low_queue: Arc<deadqueue::unlimited::Queue<AgentJob>>,
    cancel_channels: Arc<Mutex<HashMap<Uuid, watch::Sender<bool>>>>,
    project_active_counts: Arc<Mutex<HashMap<Uuid, usize>>>,
    orchestrator: Arc<ExecutorOrchestrator>,
    config: WorkerPoolConfig,
}

impl WorkerPool {
    pub fn new(orchestrator: Arc<ExecutorOrchestrator>, config: WorkerPoolConfig) -> Self {
        Self {
            high_queue: Arc::new(deadqueue::unlimited::Queue::new()),
            normal_queue: Arc::new(deadqueue::unlimited::Queue::new()),
            low_queue: Arc::new(deadqueue::unlimited::Queue::new()),
            cancel_channels: Arc::new(Mutex::new(HashMap::new())),
            project_active_counts: Arc::new(Mutex::new(HashMap::new())),
            orchestrator,
            config,
        }
    }

    /// Start worker pool with configured number of workers
    pub fn start(&self) {
        info!(
            "Starting worker pool with {} workers",
            self.config.worker_count
        );

        for worker_id in 0..self.config.worker_count {
            let high_queue = self.high_queue.clone();
            let normal_queue = self.normal_queue.clone();
            let low_queue = self.low_queue.clone();
            let cancel_channels = self.cancel_channels.clone();
            let project_active_counts = self.project_active_counts.clone();
            let orchestrator = self.orchestrator.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                info!("Worker {} started", worker_id);
                loop {
                    let job = tokio::select! {
                        biased;
                        job = high_queue.pop() => job,
                        job = normal_queue.pop() => job,
                        job = low_queue.pop() => job,
                    };

                    let project_limit = job.project_max_concurrent.max(1) as usize;
                    let current_project_active = {
                        let counts = project_active_counts.lock().await;
                        counts.get(&job.project_id).copied().unwrap_or(0)
                    };

                    if current_project_active >= project_limit {
                        // Requeue and retry shortly to enforce per-project concurrency.
                        match job.priority {
                            JobPriority::High => high_queue.push(job),
                            JobPriority::Normal => normal_queue.push(job),
                            JobPriority::Low => low_queue.push(job),
                        }
                        sleep(Duration::from_millis(200)).await;
                        continue;
                    }

                    {
                        let mut counts = project_active_counts.lock().await;
                        let count = counts.entry(job.project_id).or_insert(0);
                        *count += 1;
                    }
                    let job_project_id = job.project_id;

                    info!(
                        "Worker {} picked up {:?} priority job for attempt {}",
                        worker_id, job.priority, job.attempt_id
                    );

                    if let Err(e) = JobExecutor::process_with_retry(
                        job,
                        orchestrator.clone(),
                        cancel_channels.clone(),
                        &config,
                    )
                    .await
                    {
                        warn!("Worker {} encountered error: {}", worker_id, e);
                    }

                    {
                        let mut counts = project_active_counts.lock().await;
                        if let Some(count) = counts.get_mut(&job_project_id) {
                            *count = count.saturating_sub(1);
                            if *count == 0 {
                                counts.remove(&job_project_id);
                            }
                        }
                    }
                }
            });
        }

        info!("Worker pool started successfully");
    }

    /// Submit a job to the queue
    pub async fn submit(&self, job: AgentJob) -> Result<()> {
        info!(
            "Submitting {:?} priority job for attempt {}",
            job.priority, job.attempt_id
        );
        match job.priority {
            JobPriority::High => self.high_queue.push(job),
            JobPriority::Normal => self.normal_queue.push(job),
            JobPriority::Low => self.low_queue.push(job),
        }
        Ok(())
    }

    /// Cancel a running job by attempt ID
    pub async fn cancel(&self, attempt_id: Uuid) -> Result<()> {
        let channels = self.cancel_channels.lock().await;
        if let Some(tx) = channels.get(&attempt_id) {
            info!("Sending cancellation signal for attempt {}", attempt_id);
            tx.send(true)
                .context("Failed to send cancellation signal")?;
            Ok(())
        } else {
            warn!("No active job found for attempt {}", attempt_id);
            anyhow::bail!("No active job found for attempt {}", attempt_id)
        }
    }

    /// Get queue depth (number of pending jobs)
    pub fn queue_depth(&self) -> usize {
        self.high_queue.len() + self.normal_queue.len() + self.low_queue.len()
    }

    /// Get number of active jobs (being processed)
    pub async fn active_jobs_count(&self) -> usize {
        self.cancel_channels.lock().await.len()
    }

    /// Stop the worker pool gracefully
    pub async fn stop(&self) {
        info!("Stopping worker pool gracefully...");
        // Cancel all active jobs
        let channels = self.cancel_channels.lock().await;
        for (attempt_id, sender) in channels.iter() {
            info!("Cancelling job for attempt {}", attempt_id);
            let _ = sender.send(true);
        }
        info!("Worker pool stopped");
    }
}
