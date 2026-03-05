-- Backfill tasks previously marked as backlog via metadata-only lane flags.
-- Must run in a separate migration after enum value is committed.
UPDATE tasks
SET status = 'backlog',
    updated_at = NOW()
WHERE status = 'todo'
  AND (
    LOWER(COALESCE(metadata->>'kanban_lane', '')) = 'backlog'
    OR LOWER(COALESCE(metadata->>'kanban_column', '')) = 'backlog'
    OR LOWER(COALESCE(metadata->>'backlog', '')) IN ('true', '1', 'yes', 'on')
    OR LOWER(COALESCE(metadata->>'is_backlog', '')) IN ('true', '1', 'yes', 'on')
  );
