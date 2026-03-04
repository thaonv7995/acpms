-- S9: Deprecation notice for agent_logs (R7 Log Storage JSONL)
-- Logs are now stored in S3 JSONL (task_attempts.s3_log_key).
-- Dual-write to agent_logs is kept for backward compat; dashboard and
-- agent-activity (project_id filter) still read from this table.
-- Do NOT drop this table until all consumers migrate to S3.
COMMENT ON TABLE agent_logs IS 'DEPRECATED: Logs primary in S3 JSONL (task_attempts.s3_log_key). Kept for dual-write backward compat.';
