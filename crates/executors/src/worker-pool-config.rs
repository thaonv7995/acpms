use std::time::Duration;

/// Configuration for worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    pub worker_count: usize,
    pub job_timeout: Duration,
    pub max_retry_attempts: u32,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            worker_count: 10,
            job_timeout: Duration::from_secs(3600), // 1 hour
            max_retry_attempts: 3,
        }
    }
}

impl WorkerPoolConfig {
    pub fn new(worker_count: usize, job_timeout: Duration, max_retry_attempts: u32) -> Self {
        Self {
            worker_count,
            job_timeout,
            max_retry_attempts,
        }
    }

    pub fn with_worker_count(mut self, count: usize) -> Self {
        self.worker_count = count;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.job_timeout = timeout;
        self
    }

    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retry_attempts = retries;
        self
    }
}
