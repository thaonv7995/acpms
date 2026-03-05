-- Requirement breakdown sessions
-- Stores lifecycle and AI proposal before confirming task creation.

CREATE TABLE IF NOT EXISTS requirement_breakdown_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    requirement_id UUID NOT NULL REFERENCES requirements(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'review', 'confirmed', 'failed', 'cancelled')),

    analysis JSONB,
    impact JSONB,
    plan JSONB,
    proposed_tasks JSONB,
    suggested_sprint_id UUID REFERENCES sprints(id) ON DELETE SET NULL,

    raw_output TEXT,
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_requirement_breakdown_sessions_project_requirement
    ON requirement_breakdown_sessions(project_id, requirement_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_requirement_breakdown_sessions_status
    ON requirement_breakdown_sessions(status);

CREATE INDEX IF NOT EXISTS idx_requirement_breakdown_sessions_creator
    ON requirement_breakdown_sessions(created_by, created_at DESC);

COMMENT ON TABLE requirement_breakdown_sessions IS
    'AI requirement breakdown session lifecycle and proposed task payloads before confirmation.';
