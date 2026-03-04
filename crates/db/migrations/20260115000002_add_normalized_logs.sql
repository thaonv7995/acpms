-- Migration: Add normalized log entries table
-- Purpose: Store structured log data extracted from agent output
-- Phase 1 of vibe-kanban alignment

CREATE TABLE normalized_log_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    raw_log_id UUID REFERENCES agent_logs(id) ON DELETE SET NULL,

    -- Entry type: 'action', 'file_change', 'todo_item', 'tool_status'
    entry_type TEXT NOT NULL,

    -- JSON data for the specific entry type
    entry_data JSONB NOT NULL,

    -- Source line number from raw log
    line_number INTEGER NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for query performance
CREATE INDEX idx_normalized_logs_attempt ON normalized_log_entries(attempt_id, created_at);
CREATE INDEX idx_normalized_logs_type ON normalized_log_entries(entry_type);
CREATE INDEX idx_normalized_logs_raw ON normalized_log_entries(raw_log_id) WHERE raw_log_id IS NOT NULL;

-- Index for JSONB queries (for filtering by tool name, file path, etc.)
CREATE INDEX idx_normalized_logs_entry_data ON normalized_log_entries USING GIN (entry_data);

-- Comments for documentation
COMMENT ON TABLE normalized_log_entries IS 'Structured log entries extracted from raw agent logs for UI visualization and analytics';
COMMENT ON COLUMN normalized_log_entries.entry_type IS 'Type of normalized entry: action, file_change, todo_item, tool_status';
COMMENT ON COLUMN normalized_log_entries.entry_data IS 'JSON data matching the entry_type structure';
COMMENT ON COLUMN normalized_log_entries.line_number IS 'Line number in the raw log where this entry was extracted';
