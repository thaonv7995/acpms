-- Add subagent relationship tracking for Task tool spawns
-- Created: 2026-02-02

-- Create subagent_relationships table to track parent-child agent relationships
CREATE TABLE IF NOT EXISTS subagent_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    child_attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    spawned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    spawn_tool_use_id TEXT,
    UNIQUE(parent_attempt_id, child_attempt_id)
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_subagent_parent ON subagent_relationships(parent_attempt_id);
CREATE INDEX IF NOT EXISTS idx_subagent_child ON subagent_relationships(child_attempt_id);
CREATE INDEX IF NOT EXISTS idx_subagent_spawned_at ON subagent_relationships(spawned_at DESC);

-- Comments for documentation
COMMENT ON TABLE subagent_relationships IS 'Tracks parent-child relationships when agents spawn subagents via Task tool';
COMMENT ON COLUMN subagent_relationships.parent_attempt_id IS 'The attempt that spawned the subagent';
COMMENT ON COLUMN subagent_relationships.child_attempt_id IS 'The spawned subagent attempt';
COMMENT ON COLUMN subagent_relationships.spawn_tool_use_id IS 'The tool_use ID from the Task tool call that spawned this subagent';
COMMENT ON COLUMN subagent_relationships.spawned_at IS 'Timestamp when the subagent was spawned';
