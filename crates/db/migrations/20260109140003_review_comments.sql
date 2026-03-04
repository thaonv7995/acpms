-- Add review comments table for code review workflow
-- Phase 4: Review Workflow
-- Created: 2026-01-09

-- Create review_comments table
CREATE TABLE IF NOT EXISTS review_comments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    file_path TEXT,
    line_number INTEGER,
    content TEXT NOT NULL,
    resolved BOOLEAN NOT NULL DEFAULT false,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_review_comments_attempt ON review_comments(attempt_id);
CREATE INDEX IF NOT EXISTS idx_review_comments_user ON review_comments(user_id);
CREATE INDEX IF NOT EXISTS idx_review_comments_resolved ON review_comments(resolved);
CREATE INDEX IF NOT EXISTS idx_review_comments_file ON review_comments(file_path);
CREATE INDEX IF NOT EXISTS idx_review_comments_created_at ON review_comments(created_at DESC);

-- Composite index for file-specific comments
CREATE INDEX IF NOT EXISTS idx_review_comments_file_line
ON review_comments(attempt_id, file_path, line_number)
WHERE file_path IS NOT NULL;

-- Trigger for updated_at
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_review_comments_updated_at') THEN
        CREATE TRIGGER update_review_comments_updated_at
        BEFORE UPDATE ON review_comments
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;

-- Add comments for documentation
COMMENT ON TABLE review_comments IS 'Code review comments on task attempts, supporting line-level, file-level, and general comments';
COMMENT ON COLUMN review_comments.attempt_id IS 'Reference to the task attempt being reviewed';
COMMENT ON COLUMN review_comments.file_path IS 'Relative path to file in repository. NULL for general comments';
COMMENT ON COLUMN review_comments.line_number IS 'Line number in file. NULL for file-level or general comments';
COMMENT ON COLUMN review_comments.content IS 'Comment text content';
COMMENT ON COLUMN review_comments.resolved IS 'Whether this comment has been addressed/resolved';
COMMENT ON COLUMN review_comments.resolved_by IS 'User who marked the comment as resolved';
COMMENT ON COLUMN review_comments.resolved_at IS 'Timestamp when comment was resolved';
