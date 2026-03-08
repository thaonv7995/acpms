CREATE TABLE IF NOT EXISTS openclaw_gateway_events (
    sequence_id BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    project_id UUID NULL,
    task_id UUID NULL,
    attempt_id UUID NULL,
    source TEXT NOT NULL,
    payload JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_openclaw_events_occurred_at
    ON openclaw_gateway_events (occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_openclaw_events_attempt_id
    ON openclaw_gateway_events (attempt_id, sequence_id DESC);

CREATE INDEX IF NOT EXISTS idx_openclaw_events_task_id
    ON openclaw_gateway_events (task_id, sequence_id DESC);

CREATE INDEX IF NOT EXISTS idx_openclaw_events_project_id
    ON openclaw_gateway_events (project_id, sequence_id DESC);
