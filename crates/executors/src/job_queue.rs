use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

/// Project Assistant chat job (persistent CLI session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAssistantJob {
    pub session_id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub repo_path: PathBuf,
    pub instruction: String,
}

/// Job priority levels for task scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum JobPriority {
    High = 1, // User-triggered retry or urgent tasks
    #[default]
    Normal = 5, // Default priority
    Low = 10, // Automated background tasks
}

/// Represents a task execution job to be processed by worker pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentJob {
    pub attempt_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub repo_path: PathBuf,
    pub instruction: String,
    pub priority: JobPriority,
    /// If true, agent changes require human review before commit/push
    pub require_review: bool,
    /// Execution timeout in seconds (from project settings)
    pub timeout_secs: u64,
    /// Maximum retries allowed (from project settings)
    pub max_retries: i32,
    /// Whether auto-retry is enabled (from project settings)
    pub auto_retry: bool,
    /// Current retry count (0 for first attempt)
    pub retry_count: i32,
    /// Maximum concurrent active jobs allowed for this project
    pub project_max_concurrent: i32,
    /// Cancellation reason if cancelled
    pub cancel_reason: Option<String>,
}

impl AgentJob {
    pub fn new(
        attempt_id: Uuid,
        task_id: Uuid,
        project_id: Uuid,
        repo_path: PathBuf,
        instruction: String,
        require_review: bool,
    ) -> Self {
        Self {
            attempt_id,
            task_id,
            project_id,
            repo_path,
            instruction,
            priority: JobPriority::default(),
            require_review,
            timeout_secs: 30 * 60, // Default 30 minutes
            max_retries: 3,
            auto_retry: false,
            retry_count: 0,
            project_max_concurrent: 3,
            cancel_reason: None,
        }
    }

    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timeout(mut self, timeout_mins: i32) -> Self {
        self.timeout_secs = (timeout_mins as u64) * 60;
        self
    }

    pub fn with_retry_config(mut self, max_retries: i32, auto_retry: bool) -> Self {
        self.max_retries = max_retries;
        self.auto_retry = auto_retry;
        self
    }

    pub fn with_retry_count(mut self, retry_count: i32) -> Self {
        self.retry_count = retry_count;
        self
    }

    pub fn with_project_max_concurrent(mut self, max_concurrent: i32) -> Self {
        self.project_max_concurrent = max_concurrent.max(1);
        self
    }

    /// Get the timeout as a Duration.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Check if this job can be retried.
    pub fn can_retry(&self) -> bool {
        self.auto_retry && self.retry_count < self.max_retries
    }

    /// Create a retry job from this job.
    pub fn create_retry(&self, new_attempt_id: Uuid) -> Self {
        Self {
            attempt_id: new_attempt_id,
            retry_count: self.retry_count + 1,
            priority: JobPriority::High, // Retries get higher priority
            cancel_reason: None,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_job() -> AgentJob {
        AgentJob::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            PathBuf::from("/tmp/repo"),
            "test".to_string(),
            true,
        )
    }

    #[test]
    fn default_job_has_project_concurrency_limit() {
        let job = make_job();
        assert_eq!(job.project_max_concurrent, 3);
    }

    #[test]
    fn project_concurrency_limit_is_clamped_to_at_least_one() {
        let job = make_job().with_project_max_concurrent(0);
        assert_eq!(job.project_max_concurrent, 1);
    }

    #[test]
    fn create_retry_increments_retry_count_and_preserves_project_limit() {
        let job = make_job()
            .with_project_max_concurrent(5)
            .with_retry_config(3, true)
            .with_retry_count(1);
        let retried = job.create_retry(Uuid::new_v4());

        assert_eq!(retried.retry_count, 2);
        assert_eq!(retried.project_max_concurrent, 5);
        assert_eq!(retried.priority, JobPriority::High);
    }

    #[test]
    fn can_retry_respects_auto_retry_and_max_retries() {
        let disabled = make_job().with_retry_config(3, false).with_retry_count(0);
        assert!(!disabled.can_retry());

        let at_limit = make_job().with_retry_config(2, true).with_retry_count(2);
        assert!(!at_limit.can_retry());

        let allowed = make_job().with_retry_config(2, true).with_retry_count(1);
        assert!(allowed.can_retry());
    }
}
