//! Dedicated worker pool for Project Assistant jobs.
//! Processes ProjectAssistantJob by spawning CLI via orchestrator.

use acpms_executors::ProjectAssistantJob;
use anyhow::{bail, Result};
use futures::future::BoxFuture;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::{Mutex, Notify};
use tracing::{debug, error, info};

pub type ProjectAssistantJobHandler =
    Arc<dyn Fn(ProjectAssistantJob) -> BoxFuture<'static, ()> + Send + Sync>;

/// Worker pool for Project Assistant chat jobs.
pub struct ProjectAssistantWorkerPool {
    queue: Arc<Mutex<std::collections::VecDeque<ProjectAssistantJob>>>,
    notify: Arc<Notify>,
    stop_flag: Arc<AtomicBool>,
    in_flight: Arc<AtomicUsize>,
    worker_count: usize,
    handler: ProjectAssistantJobHandler,
}

impl ProjectAssistantWorkerPool {
    pub fn new(worker_count: usize, handler: ProjectAssistantJobHandler) -> Self {
        Self {
            queue: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            notify: Arc::new(Notify::new()),
            stop_flag: Arc::new(AtomicBool::new(false)),
            in_flight: Arc::new(AtomicUsize::new(0)),
            worker_count: worker_count.max(1),
            handler,
        }
    }

    pub fn start(&self) {
        info!(
            "Starting project assistant worker pool with {} workers",
            self.worker_count
        );

        for worker_id in 0..self.worker_count {
            let queue = self.queue.clone();
            let notify = self.notify.clone();
            let stop_flag = self.stop_flag.clone();
            let in_flight = self.in_flight.clone();
            let handler = self.handler.clone();

            tokio::spawn(async move {
                debug!(worker_id, "project assistant worker started");

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

                debug!(worker_id, "project assistant worker stopped");
            });
        }
    }

    pub async fn submit(&self, job: ProjectAssistantJob) -> Result<()> {
        if self.stop_flag.load(Ordering::SeqCst) {
            bail!("project assistant worker pool is stopping");
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

        let mut attempts = 0;
        while self.in_flight.load(Ordering::SeqCst) > 0 && attempts < 50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            attempts += 1;
        }

        if self.in_flight.load(Ordering::SeqCst) > 0 {
            error!(
                in_flight = self.in_flight.load(Ordering::SeqCst),
                "project assistant worker pool stopped with in-flight jobs"
            );
        }
    }
}
