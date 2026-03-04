-- Optimize reverse infinite-scroll queries:
-- WHERE attempt_id = ? AND created_at < ? ORDER BY created_at DESC, id DESC LIMIT N
CREATE INDEX IF NOT EXISTS idx_agent_logs_attempt_created_at_desc
    ON agent_logs (attempt_id, created_at DESC, id DESC);
