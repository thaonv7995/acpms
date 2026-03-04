#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_worker_pool_config_defaults() {
        let config = WorkerPoolConfig::default();
        assert_eq!(config.worker_count, 10);
        assert_eq!(config.job_timeout, Duration::from_secs(3600));
        assert_eq!(config.max_retry_attempts, 3);
    }

    #[test]
    fn test_worker_pool_config_builder() {
        let config = WorkerPoolConfig::default()
            .with_worker_count(5)
            .with_timeout(Duration::from_secs(1800))
            .with_max_retries(5);

        assert_eq!(config.worker_count, 5);
        assert_eq!(config.job_timeout, Duration::from_secs(1800));
        assert_eq!(config.max_retry_attempts, 5);
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::High < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::Low);
    }

    #[test]
    fn test_agent_job_creation() {
        use std::path::PathBuf;
        use uuid::Uuid;

        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let repo_path = PathBuf::from("/test/repo");
        let instruction = "Test instruction".to_string();

        let job = AgentJob::new(attempt_id, task_id, repo_path.clone(), instruction.clone());

        assert_eq!(job.attempt_id, attempt_id);
        assert_eq!(job.task_id, task_id);
        assert_eq!(job.repo_path, repo_path);
        assert_eq!(job.instruction, instruction);
        assert_eq!(job.priority, JobPriority::Normal);
    }

    #[test]
    fn test_agent_job_with_priority() {
        use std::path::PathBuf;
        use uuid::Uuid;

        let attempt_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let repo_path = PathBuf::from("/test/repo");
        let instruction = "Test instruction".to_string();

        let job = AgentJob::new(attempt_id, task_id, repo_path, instruction)
            .with_priority(JobPriority::High);

        assert_eq!(job.priority, JobPriority::High);
    }
}
