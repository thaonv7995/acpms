-- Add S3 diff metadata columns to task_attempts table
-- This migration enables storing diff snapshots in MinIO/S3 instead of database
-- for better scalability and cost efficiency

-- Add columns for S3-based diff storage
ALTER TABLE task_attempts
ADD COLUMN IF NOT EXISTS s3_diff_key TEXT,
ADD COLUMN IF NOT EXISTS s3_diff_size BIGINT,
ADD COLUMN IF NOT EXISTS s3_diff_saved_at TIMESTAMPTZ;

-- Create index for querying attempts with saved diffs
CREATE INDEX IF NOT EXISTS idx_task_attempts_s3_diff_key
ON task_attempts(s3_diff_key)
WHERE s3_diff_key IS NOT NULL;

-- Create index for cleanup/archival queries
CREATE INDEX IF NOT EXISTS idx_task_attempts_diff_saved_at
ON task_attempts(s3_diff_saved_at)
WHERE s3_diff_saved_at IS NOT NULL;

-- Add comments for documentation
COMMENT ON COLUMN task_attempts.s3_diff_key IS 'S3 object key for stored diff snapshot (e.g., diffs/2026/01/14/{attempt_id}.json)';
COMMENT ON COLUMN task_attempts.s3_diff_size IS 'Size in bytes of the JSON diff snapshot stored in S3';
COMMENT ON COLUMN task_attempts.s3_diff_saved_at IS 'Timestamp when diff snapshot was saved to S3';
