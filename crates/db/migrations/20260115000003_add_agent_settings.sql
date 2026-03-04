-- Migration: Add agent settings to projects
-- Purpose: Configure router service and agent behavior per project
-- Phase 2 of vibe-kanban alignment

-- Add agent_settings column to projects table
ALTER TABLE projects ADD COLUMN IF NOT EXISTS agent_settings JSONB NOT NULL DEFAULT '{
  "enable_router_service": false,
  "router_version": "1.0.66",
  "router_filters": [],
  "router_timeout_ms": 5000
}'::jsonb;

-- Add failure_reason column to task_attempts for router crash tracking
ALTER TABLE task_attempts ADD COLUMN IF NOT EXISTS failure_reason TEXT;

COMMENT ON COLUMN projects.agent_settings IS 'Agent configuration: router service, filters, timeouts';
COMMENT ON COLUMN task_attempts.failure_reason IS 'Categorizes failure: agent_error, router_crashed, timeout, user_cancelled';

-- Create index for failure analysis
CREATE INDEX IF NOT EXISTS idx_task_attempts_failure_reason
ON task_attempts(failure_reason) WHERE failure_reason IS NOT NULL;

-- Security events table for approval spoofing detection (Phase 6 prep)
CREATE TABLE IF NOT EXISTS security_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_security_events_attempt ON security_events(attempt_id);
CREATE INDEX idx_security_events_type ON security_events(event_type);
CREATE INDEX idx_security_events_created ON security_events(created_at DESC);

COMMENT ON TABLE security_events IS 'Security audit trail for approval spoofing and other threats';
