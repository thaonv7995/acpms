-- Scope tool approvals by execution process (process-first approval lifecycle)

ALTER TABLE tool_approvals
    ADD COLUMN IF NOT EXISTS execution_process_id UUID REFERENCES execution_processes(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tool_approvals_execution_process_id
    ON tool_approvals(execution_process_id);

-- Backfill existing approval rows to the closest process window of the same attempt.
WITH process_windows AS (
    SELECT
        ep.id,
        ep.attempt_id,
        ep.created_at,
        LEAD(ep.created_at) OVER (
            PARTITION BY ep.attempt_id
            ORDER BY ep.created_at ASC, ep.id ASC
        ) AS next_created_at
    FROM execution_processes ep
)
UPDATE tool_approvals ta
SET execution_process_id = pw.id
FROM process_windows pw
WHERE ta.execution_process_id IS NULL
  AND ta.attempt_id = pw.attempt_id
  AND ta.created_at >= pw.created_at
  AND (pw.next_created_at IS NULL OR ta.created_at < pw.next_created_at);
