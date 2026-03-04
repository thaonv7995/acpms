-- Migration: Add performance indexes for kanban queries
-- Task 002: TaskWithAttemptStatus Query
-- Phase 0: Backend Data Models

-- Index for has_in_progress_attempt subquery
-- Speeds up EXISTS(SELECT 1 FROM task_attempts WHERE task_id = t.id AND status = 'running')
CREATE INDEX IF NOT EXISTS idx_task_attempts_task_status
ON task_attempts(task_id, status);

-- Index for last_attempt_failed and executor subqueries
-- Speeds up ORDER BY created_at DESC LIMIT 1 queries
CREATE INDEX IF NOT EXISTS idx_task_attempts_task_created
ON task_attempts(task_id, created_at DESC);

-- Update statistics for query planner
ANALYZE task_attempts;
