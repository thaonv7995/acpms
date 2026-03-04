//! Retry Handler Service
//!
//! Manages automatic retry logic for failed task attempts with exponential backoff.
//! Integrates with project settings for configurable retry behavior.

use acpms_db::models::{AttemptStatus, ProjectSettings, TaskAttempt};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Retry handler service for managing task attempt retries with exponential backoff.
///
/// ## Configuration
/// - `max_retries`: Maximum number of retry attempts (from project settings)
/// - `backoff_base`: Base duration for exponential backoff (default: 60 seconds)
/// - `auto_retry`: Whether to automatically retry failed tasks (from project settings)
///
/// ## Backoff Strategy
/// Uses exponential backoff: backoff = base * 2^retry_count
/// - Retry 1: 1 minute
/// - Retry 2: 2 minutes
/// - Retry 3: 4 minutes
#[derive(Debug, Clone)]
pub struct RetryHandler {
    max_retries: i32,
    backoff_base: Duration,
    auto_retry: bool,
}

impl RetryHandler {
    /// Create a new RetryHandler from project settings.
    pub fn new(settings: &ProjectSettings) -> Self {
        Self {
            max_retries: settings.max_retries,
            backoff_base: Duration::from_secs(60), // 1 minute base
            auto_retry: settings.auto_retry,
        }
    }

    /// Create a RetryHandler with custom configuration.
    pub fn with_config(max_retries: i32, backoff_base_secs: u64, auto_retry: bool) -> Self {
        Self {
            max_retries,
            backoff_base: Duration::from_secs(backoff_base_secs),
            auto_retry,
        }
    }

    /// Check if auto-retry is enabled for this project.
    pub fn is_auto_retry_enabled(&self) -> bool {
        self.auto_retry
    }

    /// Check if the attempt should be retried based on retry count.
    ///
    /// ## Arguments
    /// - `attempt`: The task attempt to check
    ///
    /// ## Returns
    /// - `true` if retry count is less than max_retries
    /// - `false` otherwise
    pub fn should_retry(&self, attempt: &TaskAttempt) -> bool {
        let retry_count = self.get_retry_count(attempt);
        retry_count < self.max_retries
    }

    /// Get the current retry count from attempt metadata.
    pub fn get_retry_count(&self, attempt: &TaskAttempt) -> i32 {
        attempt
            .metadata
            .get("retry_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32
    }

    /// Calculate the backoff duration for a given retry count.
    ///
    /// Uses exponential backoff: base * 2^retry_count
    pub fn get_backoff(&self, retry_count: i32) -> Duration {
        self.backoff_base * 2u32.pow(retry_count as u32)
    }

    /// Get remaining retries for an attempt.
    pub fn get_remaining_retries(&self, attempt: &TaskAttempt) -> i32 {
        let retry_count = self.get_retry_count(attempt);
        (self.max_retries - retry_count).max(0)
    }

    /// Get the next retry time based on current retry count.
    pub fn get_next_retry_time(&self, attempt: &TaskAttempt) -> DateTime<Utc> {
        let retry_count = self.get_retry_count(attempt);
        let backoff = self.get_backoff(retry_count);
        Utc::now() + chrono::Duration::from_std(backoff).unwrap_or_default()
    }

    /// Check if the error is retriable (certain errors should not trigger retry).
    pub fn is_retriable_error(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Non-retriable errors
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

        !non_retriable.iter().any(|e| error_lower.contains(e))
    }

    /// Create metadata for a retry attempt.
    ///
    /// ## Arguments
    /// - `previous_attempt`: The failed attempt being retried
    /// - `error`: The error message from the failed attempt
    ///
    /// ## Returns
    /// JSON metadata for the new attempt including retry information
    pub fn create_retry_metadata(
        &self,
        previous_attempt: &TaskAttempt,
        error: &str,
    ) -> serde_json::Value {
        let retry_count = self.get_retry_count(previous_attempt) + 1;

        let mut metadata = serde_json::json!({
            "retry_count": retry_count,
            "previous_attempt_id": previous_attempt.id,
            "previous_error": error,
            "retry_scheduled_at": Utc::now().to_rfc3339(),
            "backoff_seconds": self.get_backoff(retry_count).as_secs(),
        });

        if let Some(chain) = previous_attempt
            .metadata
            .get("resolved_skill_chain")
            .filter(|value| value.is_array())
            .cloned()
        {
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert("resolved_skill_chain".to_string(), chain);
                obj.insert(
                    "resolved_skill_chain_source".to_string(),
                    serde_json::Value::String("retry_inherit_previous_attempt".to_string()),
                );
            }
        }

        metadata
    }

    /// Schedule a retry for a failed task attempt.
    ///
    /// ## Arguments
    /// - `pool`: Database connection pool
    /// - `task_id`: The task to retry
    /// - `previous_attempt`: The failed attempt
    /// - `error`: The error message
    ///
    /// ## Returns
    /// The ID of the newly created retry attempt
    pub async fn schedule_retry(
        &self,
        pool: &PgPool,
        task_id: Uuid,
        previous_attempt: &TaskAttempt,
        error: &str,
    ) -> Result<RetryScheduleResult> {
        let retry_count = self.get_retry_count(previous_attempt) + 1;

        if retry_count > self.max_retries {
            return Ok(RetryScheduleResult::MaxRetriesExceeded {
                retry_count,
                max_retries: self.max_retries,
            });
        }

        if !self.is_retriable_error(error) {
            return Ok(RetryScheduleResult::NonRetriableError {
                error: error.to_string(),
            });
        }

        let backoff = self.get_backoff(retry_count);
        let retry_at = Utc::now() + chrono::Duration::from_std(backoff).unwrap_or_default();
        let metadata = self.create_retry_metadata(previous_attempt, error);

        // Create new attempt record with retry metadata
        let new_attempt = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO task_attempts (task_id, status, metadata)
            VALUES ($1, 'queued', $2)
            RETURNING id
            "#,
        )
        .bind(task_id)
        .bind(&metadata)
        .fetch_one(pool)
        .await
        .context("Failed to create retry attempt")?;

        info!(
            "Scheduled retry {} for task {} (attempt {}), backoff: {:?}, retry_at: {}",
            retry_count, task_id, new_attempt, backoff, retry_at
        );

        Ok(RetryScheduleResult::Scheduled {
            attempt_id: new_attempt,
            retry_count,
            backoff,
            retry_at,
        })
    }

    /// Update the previous attempt with retry information.
    pub async fn mark_attempt_for_retry(
        &self,
        pool: &PgPool,
        attempt_id: Uuid,
        new_attempt_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object('retry_attempt_id', $2::text)
            WHERE id = $1
            "#,
        )
        .bind(attempt_id)
        .bind(new_attempt_id.to_string())
        .execute(pool)
        .await
        .context("Failed to update attempt with retry info")?;

        Ok(())
    }
}

/// Result of scheduling a retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum RetryScheduleResult {
    /// Retry was successfully scheduled.
    Scheduled {
        attempt_id: Uuid,
        retry_count: i32,
        #[serde(with = "duration_serde")]
        backoff: Duration,
        retry_at: DateTime<Utc>,
    },
    /// Maximum retries exceeded.
    MaxRetriesExceeded { retry_count: i32, max_retries: i32 },
    /// Error is not retriable.
    NonRetriableError { error: String },
}

/// Retry information for an attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryInfo {
    /// Current retry count.
    pub retry_count: i32,
    /// Maximum retries allowed.
    pub max_retries: i32,
    /// Remaining retries.
    pub remaining_retries: i32,
    /// Whether the attempt can be retried.
    pub can_retry: bool,
    /// Whether auto-retry is enabled.
    pub auto_retry_enabled: bool,
    /// Previous attempt ID (if this is a retry).
    pub previous_attempt_id: Option<Uuid>,
    /// Previous error message (if this is a retry).
    pub previous_error: Option<String>,
    /// Next retry attempt ID (if a retry is scheduled).
    pub next_retry_attempt_id: Option<Uuid>,
    /// Next backoff duration in seconds (if retry is possible).
    pub next_backoff_seconds: Option<u64>,
}

impl RetryInfo {
    /// Create retry info from an attempt and settings.
    pub fn from_attempt(attempt: &TaskAttempt, settings: &ProjectSettings) -> Self {
        let handler = RetryHandler::new(settings);
        let retry_count = handler.get_retry_count(attempt);
        let remaining = handler.get_remaining_retries(attempt);
        let is_retriable_status = matches!(
            attempt.status,
            AttemptStatus::Failed | AttemptStatus::Cancelled
        );
        let can_retry = is_retriable_status && retry_count < settings.max_retries;

        let previous_attempt_id = attempt
            .metadata
            .get("previous_attempt_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let previous_error = attempt
            .metadata
            .get("previous_error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let next_retry_attempt_id = attempt
            .metadata
            .get("retry_attempt_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let next_backoff_seconds = if can_retry && matches!(attempt.status, AttemptStatus::Failed) {
            Some(handler.get_backoff(retry_count).as_secs())
        } else {
            None
        };

        Self {
            retry_count,
            max_retries: settings.max_retries,
            remaining_retries: remaining,
            can_retry,
            auto_retry_enabled: settings.auto_retry,
            previous_attempt_id,
            previous_error,
            next_retry_attempt_id,
            next_backoff_seconds,
        }
    }
}

/// Custom serde for Duration.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_settings() -> ProjectSettings {
        ProjectSettings {
            max_retries: 3,
            auto_retry: true,
            ..Default::default()
        }
    }

    fn create_test_attempt(retry_count: i32, status: AttemptStatus) -> TaskAttempt {
        TaskAttempt {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            status,
            started_at: None,
            completed_at: None,
            error_message: Some("Test error".to_string()),
            metadata: serde_json::json!({ "retry_count": retry_count }),
            created_at: Utc::now(),
            diff_total_files: None,
            diff_total_additions: None,
            diff_total_deletions: None,
            diff_saved_at: None,
            s3_diff_key: None,
            s3_diff_size: None,
            s3_diff_saved_at: None,
            s3_log_key: None,
        }
    }

    #[test]
    fn test_should_retry_within_limit() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);

        let attempt = create_test_attempt(0, AttemptStatus::Failed);
        assert!(handler.should_retry(&attempt));

        let attempt = create_test_attempt(2, AttemptStatus::Failed);
        assert!(handler.should_retry(&attempt));
    }

    #[test]
    fn test_should_not_retry_at_limit() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);

        let attempt = create_test_attempt(3, AttemptStatus::Failed);
        assert!(!handler.should_retry(&attempt));

        let attempt = create_test_attempt(5, AttemptStatus::Failed);
        assert!(!handler.should_retry(&attempt));
    }

    #[test]
    fn test_get_backoff_exponential() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);

        assert_eq!(handler.get_backoff(0).as_secs(), 60); // 1 min
        assert_eq!(handler.get_backoff(1).as_secs(), 120); // 2 min
        assert_eq!(handler.get_backoff(2).as_secs(), 240); // 4 min
        assert_eq!(handler.get_backoff(3).as_secs(), 480); // 8 min
    }

    #[test]
    fn test_remaining_retries() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);

        assert_eq!(
            handler.get_remaining_retries(&create_test_attempt(0, AttemptStatus::Failed)),
            3
        );
        assert_eq!(
            handler.get_remaining_retries(&create_test_attempt(1, AttemptStatus::Failed)),
            2
        );
        assert_eq!(
            handler.get_remaining_retries(&create_test_attempt(3, AttemptStatus::Failed)),
            0
        );
        assert_eq!(
            handler.get_remaining_retries(&create_test_attempt(5, AttemptStatus::Failed)),
            0
        );
    }

    #[test]
    fn test_is_retriable_error() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);

        // Retriable errors
        assert!(handler.is_retriable_error("Connection timeout"));
        assert!(handler.is_retriable_error("Network error"));
        assert!(handler.is_retriable_error("Internal server error"));

        // Non-retriable errors
        assert!(!handler.is_retriable_error("Permission denied"));
        assert!(!handler.is_retriable_error("Authentication failed"));
        assert!(!handler.is_retriable_error("Cancelled by user"));
        assert!(!handler.is_retriable_error("Rate limit exceeded"));
    }

    #[test]
    fn test_create_retry_metadata() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);
        let attempt = create_test_attempt(1, AttemptStatus::Failed);

        let metadata = handler.create_retry_metadata(&attempt, "Test error");

        assert_eq!(metadata["retry_count"], 2);
        assert_eq!(metadata["previous_attempt_id"], attempt.id.to_string());
        assert_eq!(metadata["previous_error"], "Test error");
        assert_eq!(metadata["backoff_seconds"], 240); // 4 minutes for retry 2
    }

    #[test]
    fn test_create_retry_metadata_carries_resolved_skill_chain() {
        let settings = create_test_settings();
        let handler = RetryHandler::new(&settings);
        let mut attempt = create_test_attempt(0, AttemptStatus::Failed);
        attempt.metadata = serde_json::json!({
            "retry_count": 0,
            "resolved_skill_chain": ["env-and-secrets-validate", "code-implement"]
        });

        let metadata = handler.create_retry_metadata(&attempt, "Transient network error");

        assert_eq!(
            metadata["resolved_skill_chain"],
            serde_json::json!(["env-and-secrets-validate", "code-implement"])
        );
        assert_eq!(
            metadata["resolved_skill_chain_source"],
            "retry_inherit_previous_attempt"
        );
    }

    #[test]
    fn test_retry_info_allows_cancelled_attempts() {
        let settings = create_test_settings();
        let attempt = create_test_attempt(1, AttemptStatus::Cancelled);

        let info = RetryInfo::from_attempt(&attempt, &settings);

        assert!(info.can_retry);
        assert_eq!(info.remaining_retries, 2);
        assert_eq!(info.next_backoff_seconds, None);
    }
}
