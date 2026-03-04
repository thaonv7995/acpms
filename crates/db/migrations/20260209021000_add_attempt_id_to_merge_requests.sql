-- Track which attempt created each merge request so approve/merge can target the correct branch.
ALTER TABLE merge_requests
ADD COLUMN IF NOT EXISTS attempt_id UUID REFERENCES task_attempts(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_merge_requests_attempt_id
ON merge_requests(attempt_id);
