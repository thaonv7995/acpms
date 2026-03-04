-- Add S3 log key column for JSONL log storage (R7 - Vibe Kanban parity)
-- Attempts with s3_log_key load logs from S3; others use agent_logs (backward compat)

ALTER TABLE task_attempts
ADD COLUMN IF NOT EXISTS s3_log_key TEXT;

CREATE INDEX IF NOT EXISTS idx_task_attempts_s3_log_key
ON task_attempts(s3_log_key)
WHERE s3_log_key IS NOT NULL;

COMMENT ON COLUMN task_attempts.s3_log_key IS 'S3 object key for attempt logs JSONL (e.g., attempts/{attempt_id}/logs.jsonl)';
