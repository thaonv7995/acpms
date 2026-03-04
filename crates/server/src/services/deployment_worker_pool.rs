use anyhow::{bail, Result};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{Mutex, Notify};
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Deployment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentJob {
    pub kind: JobKind,
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
}

impl DeploymentJob {
    pub fn new(run_id: Uuid, project_id: Uuid, environment_id: Uuid) -> Self {
        Self {
            kind: JobKind::Deployment,
            run_id,
            project_id,
            environment_id,
        }
    }
}

pub type DeploymentJobHandler = Arc<dyn Fn(DeploymentJob) -> BoxFuture<'static, ()> + Send + Sync>;

/// Dedicated worker pool for deployment jobs.
///
/// This decouples deployment execution from request threads and provides
/// queue-based processing semantics similar to Agent worker pool.
pub struct DeploymentWorkerPool {
    queue: Arc<Mutex<VecDeque<DeploymentJob>>>,
    notify: Arc<Notify>,
    stop_flag: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
    worker_count: usize,
    handler: DeploymentJobHandler,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn submit_dispatches_job_to_handler() {
        let seen = Arc::new(Mutex::new(Vec::<Uuid>::new()));
        let processed = Arc::new(Notify::new());

        let handler_seen = seen.clone();
        let handler_processed = processed.clone();
        let handler: DeploymentJobHandler = Arc::new(move |job: DeploymentJob| {
            let handler_seen = handler_seen.clone();
            let handler_processed = handler_processed.clone();
            Box::pin(async move {
                handler_seen.lock().await.push(job.run_id);
                handler_processed.notify_one();
            })
        });

        let pool = DeploymentWorkerPool::new(1, handler);
        pool.start();

        let run_id = Uuid::new_v4();
        pool.submit(DeploymentJob::new(run_id, Uuid::new_v4(), Uuid::new_v4()))
            .await
            .expect("submit should succeed");

        tokio::time::timeout(Duration::from_secs(1), processed.notified())
            .await
            .expect("job should be processed");
        assert_eq!(seen.lock().await.as_slice(), &[run_id]);

        pool.stop().await;
    }

    #[tokio::test]
    async fn queue_depth_counts_in_flight_job() {
        let job_started = Arc::new(Notify::new());
        let release_job = Arc::new(Notify::new());

        let handler_job_started = job_started.clone();
        let handler_release_job = release_job.clone();
        let handler: DeploymentJobHandler = Arc::new(move |_job: DeploymentJob| {
            let handler_job_started = handler_job_started.clone();
            let handler_release_job = handler_release_job.clone();
            Box::pin(async move {
                handler_job_started.notify_one();
                handler_release_job.notified().await;
            })
        });

        let pool = DeploymentWorkerPool::new(1, handler);
        pool.start();

        pool.submit(DeploymentJob::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ))
        .await
        .expect("submit should succeed");

        tokio::time::timeout(Duration::from_secs(1), job_started.notified())
            .await
            .expect("job should start");

        let depth = pool.queue_depth().await;
        assert_eq!(depth, 1);

        release_job.notify_waiters();
        pool.stop().await;
    }

    #[tokio::test]
    async fn stop_rejects_new_jobs() {
        let handler: DeploymentJobHandler = Arc::new(|_job: DeploymentJob| Box::pin(async move {}));
        let pool = DeploymentWorkerPool::new(1, handler);
        pool.start();
        pool.stop().await;

        let submit_result = pool
            .submit(DeploymentJob::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
            ))
            .await;
        assert!(submit_result.is_err(), "submit after stop should fail");
    }
}

impl DeploymentWorkerPool {
    pub fn new(worker_count: usize, handler: DeploymentJobHandler) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
            stop_flag: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(AtomicUsize::new(0)),
            worker_count: worker_count.max(1),
            handler,
        }
    }

    pub fn start(&self) {
        info!(
            "Starting deployment worker pool with {} workers",
            self.worker_count
        );

        for worker_id in 0..self.worker_count {
            let queue = self.queue.clone();
            let notify = self.notify.clone();
            let stop_flag = self.stop_flag.clone();
            let in_flight = self.in_flight.clone();
            let handler = self.handler.clone();

            tokio::spawn(async move {
                debug!(worker_id, "deployment worker started");

                loop {
                    let maybe_job = {
                        let mut guard = queue.lock().await;
                        guard.pop_front()
                    };

                    if let Some(job) = maybe_job {
                        in_flight.fetch_add(1, Ordering::SeqCst);
                        (handler)(job).await;
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                        continue;
                    }

                    if stop_flag.load(Ordering::SeqCst) {
                        break;
                    }

                    notify.notified().await;
                }

                debug!(worker_id, "deployment worker stopped");
            });
        }
    }

    pub async fn submit(&self, job: DeploymentJob) -> Result<()> {
        if self.stop_flag.load(Ordering::SeqCst) {
            bail!("deployment worker pool is stopping");
        }

        {
            let mut guard = self.queue.lock().await;
            guard.push_back(job);
        }

        self.notify.notify_one();
        Ok(())
    }

    pub async fn queue_depth(&self) -> usize {
        let queued = self.queue.lock().await.len();
        queued + self.in_flight.load(Ordering::SeqCst)
    }

    pub async fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();

        // Best-effort grace period for workers to exit.
        let mut attempts = 0;
        while self.in_flight.load(Ordering::SeqCst) > 0 && attempts < 50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            attempts += 1;
        }

        if self.in_flight.load(Ordering::SeqCst) > 0 {
            error!(
                in_flight = self.in_flight.load(Ordering::SeqCst),
                "deployment worker pool stopped with in-flight jobs"
            );
        }
    }
}
