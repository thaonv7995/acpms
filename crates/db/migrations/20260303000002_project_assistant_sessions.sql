-- Project Assistant Chat sessions
-- 1 active session per user per project (enforced by UNIQUE partial index)
CREATE TABLE project_assistant_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'ended')),
  s3_log_key TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ended_at TIMESTAMPTZ
);

CREATE INDEX idx_project_assistant_sessions_project_user
  ON project_assistant_sessions(project_id, user_id);

-- UNIQUE partial index: enforce 1 active session per user per project at DB level
CREATE UNIQUE INDEX idx_project_assistant_sessions_one_active_per_user
  ON project_assistant_sessions(project_id, user_id) WHERE status = 'active';

COMMENT ON TABLE project_assistant_sessions IS 'Chat sessions with Project Assistant. Logs stored in S3 JSONL (s3_log_key).';
COMMENT ON COLUMN project_assistant_sessions.s3_log_key IS 'S3 key: assistant-logs/{session_id}.jsonl';
