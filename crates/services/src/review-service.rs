//! Review Service for code review workflow
//!
//! Handles review comments, approval, rejection, and request-changes operations
//! for the Phase 4 review workflow.

use acpms_db::{models::*, PgPool};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::TaskAttemptService;

pub struct ReviewService {
    pool: PgPool,
}

impl ReviewService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Add a review comment to an attempt
    pub async fn add_comment(
        &self,
        attempt_id: Uuid,
        user_id: Uuid,
        req: AddReviewCommentRequest,
    ) -> Result<ReviewComment> {
        // Validate attempt exists
        let attempt_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM task_attempts WHERE id = $1)",
        )
        .bind(attempt_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check attempt existence")?;

        if !attempt_exists {
            return Err(anyhow!("Task attempt not found"));
        }

        let comment = sqlx::query_as::<_, ReviewComment>(
            r#"
            INSERT INTO review_comments (attempt_id, user_id, file_path, line_number, content)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, attempt_id, user_id, file_path, line_number, content, resolved,
                      resolved_by, resolved_at, created_at, updated_at
            "#,
        )
        .bind(attempt_id)
        .bind(user_id)
        .bind(&req.file_path)
        .bind(req.line_number)
        .bind(&req.content)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create review comment")?;

        Ok(comment)
    }

    /// Get all comments for an attempt
    pub async fn get_comments(&self, attempt_id: Uuid) -> Result<Vec<ReviewComment>> {
        let comments = sqlx::query_as::<_, ReviewComment>(
            r#"
            SELECT id, attempt_id, user_id, file_path, line_number, content, resolved,
                   resolved_by, resolved_at, created_at, updated_at
            FROM review_comments
            WHERE attempt_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(attempt_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch review comments")?;

        Ok(comments)
    }

    /// Get a single comment by ID
    pub async fn get_comment(&self, comment_id: Uuid) -> Result<Option<ReviewComment>> {
        let comment = sqlx::query_as::<_, ReviewComment>(
            r#"
            SELECT id, attempt_id, user_id, file_path, line_number, content, resolved,
                   resolved_by, resolved_at, created_at, updated_at
            FROM review_comments
            WHERE id = $1
            "#,
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch review comment")?;

        Ok(comment)
    }

    /// Resolve a comment
    pub async fn resolve_comment(&self, comment_id: Uuid, user_id: Uuid) -> Result<ReviewComment> {
        let comment = sqlx::query_as::<_, ReviewComment>(
            r#"
            UPDATE review_comments
            SET resolved = true,
                resolved_by = $2,
                resolved_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, attempt_id, user_id, file_path, line_number, content, resolved,
                      resolved_by, resolved_at, created_at, updated_at
            "#,
        )
        .bind(comment_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to resolve comment")?
        .ok_or_else(|| anyhow!("Comment not found"))?;

        Ok(comment)
    }

    /// Unresolve a comment
    pub async fn unresolve_comment(&self, comment_id: Uuid) -> Result<ReviewComment> {
        let comment = sqlx::query_as::<_, ReviewComment>(
            r#"
            UPDATE review_comments
            SET resolved = false,
                resolved_by = NULL,
                resolved_at = NULL,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, attempt_id, user_id, file_path, line_number, content, resolved,
                      resolved_by, resolved_at, created_at, updated_at
            "#,
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to unresolve comment")?
        .ok_or_else(|| anyhow!("Comment not found"))?;

        Ok(comment)
    }

    /// Delete a comment (only the author can delete)
    pub async fn delete_comment(&self, comment_id: Uuid, user_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM review_comments
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(comment_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete comment")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!(
                "Comment not found or you don't have permission to delete it"
            ));
        }

        Ok(())
    }

    /// Get unresolved comment count for an attempt
    pub async fn get_unresolved_count(&self, attempt_id: Uuid) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM review_comments WHERE attempt_id = $1 AND resolved = false",
        )
        .bind(attempt_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count unresolved comments")?;

        Ok(count)
    }

    /// Format review comments as feedback text for agent context
    pub async fn format_comments_as_feedback(&self, attempt_id: Uuid) -> Result<String> {
        let comments = self.get_comments(attempt_id).await?;

        if comments.is_empty() {
            return Ok(String::new());
        }

        let mut feedback = String::from("\n--- Review Comments ---\n");

        for comment in &comments {
            let location = match (&comment.file_path, comment.line_number) {
                (Some(path), Some(line)) => format!("{}:{}", path, line),
                (Some(path), None) => format!("{} (file-level)", path),
                (None, _) => "General".to_string(),
            };

            let resolved_mark = if comment.resolved { " [RESOLVED]" } else { "" };
            feedback.push_str(&format!(
                "\n[{}]{}\n{}\n",
                location, resolved_mark, comment.content
            ));
        }

        feedback.push_str("--- End Review Comments ---\n");

        Ok(feedback)
    }

    /// Create a new attempt with review feedback (for request-changes flow)
    /// Returns the new attempt ID
    pub async fn create_attempt_with_feedback(
        &self,
        original_attempt_id: Uuid,
        task_id: Uuid,
        feedback: &str,
        include_comments: bool,
    ) -> Result<(Uuid, i32)> {
        // Get comments if requested
        let (comments_feedback, comment_count) = if include_comments {
            let comments = self.get_comments(original_attempt_id).await?;
            let count = comments.len() as i32;
            let formatted = self
                .format_comments_as_feedback(original_attempt_id)
                .await?;
            (formatted, count)
        } else {
            (String::new(), 0)
        };

        // Build combined feedback
        let full_feedback = if comments_feedback.is_empty() {
            feedback.to_string()
        } else {
            format!("{}\n{}", feedback, comments_feedback)
        };

        // Create metadata with review feedback
        let metadata = serde_json::json!({
            "review_feedback": full_feedback,
            "original_attempt_id": original_attempt_id.to_string(),
            "changes_requested": true,
            "comments_included": comment_count,
        });

        // Create new attempt with active-attempt guard and full TaskAttempt projection
        let attempt_service = TaskAttemptService::new(self.pool.clone());
        let new_attempt = attempt_service
            .create_attempt_with_status_and_metadata(task_id, AttemptStatus::Queued, metadata)
            .await
            .context("Failed to create new attempt with feedback")?;

        // Update original attempt metadata to mark it as having changes requested
        sqlx::query(
            r#"
            UPDATE task_attempts
            SET metadata = metadata || jsonb_build_object(
                'changes_requested_at', $2::text,
                'next_attempt_id', $3::text
            )
            WHERE id = $1
            "#,
        )
        .bind(original_attempt_id)
        .bind(Utc::now().to_rfc3339())
        .bind(new_attempt.id.to_string())
        .execute(&self.pool)
        .await
        .context("Failed to update original attempt metadata")?;

        Ok((new_attempt.id, comment_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_location() {
        // Test location formatting logic
        let file_path = Some("src/main.rs".to_string());
        let line_number = Some(42);

        let location = match (&file_path, line_number) {
            (Some(path), Some(line)) => format!("{}:{}", path, line),
            (Some(path), None) => format!("{} (file-level)", path),
            (None, _) => "General".to_string(),
        };

        assert_eq!(location, "src/main.rs:42");
    }
}
