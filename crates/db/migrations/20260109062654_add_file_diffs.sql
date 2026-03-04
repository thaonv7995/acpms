-- Add file_diffs table to store diffs persistently
-- This allows retrieving diffs even after worktree cleanup

CREATE TABLE IF NOT EXISTS file_diffs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    old_path TEXT,
    change_type TEXT NOT NULL CHECK (change_type IN ('added', 'modified', 'deleted', 'renamed')),
    additions INTEGER NOT NULL DEFAULT 0,
    deletions INTEGER NOT NULL DEFAULT 0,
    old_content TEXT,
    new_content TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add summary columns to task_attempts for quick access
ALTER TABLE task_attempts
ADD COLUMN IF NOT EXISTS diff_total_files INTEGER,
ADD COLUMN IF NOT EXISTS diff_total_additions INTEGER,
ADD COLUMN IF NOT EXISTS diff_total_deletions INTEGER,
ADD COLUMN IF NOT EXISTS diff_saved_at TIMESTAMPTZ;

-- Index for fast lookup by attempt_id
CREATE INDEX IF NOT EXISTS idx_file_diffs_attempt_id ON file_diffs(attempt_id);

-- Comment
COMMENT ON TABLE file_diffs IS 'Stores git file diffs for task attempts, persisted even after worktree cleanup';
COMMENT ON COLUMN task_attempts.diff_saved_at IS 'Timestamp when diff was saved to database';
