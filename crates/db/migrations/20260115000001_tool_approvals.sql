-- Tool approval system for SDK control mode
-- This enables user approval workflow for Claude agent tool execution

-- Create approval status enum
CREATE TYPE approval_status AS ENUM (
    'pending',    -- Waiting for user response
    'approved',   -- User approved the tool execution
    'denied',     -- User denied the tool execution
    'timed_out'   -- Approval request timed out
);

-- Create tool approvals table
CREATE TABLE tool_approvals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    tool_use_id VARCHAR(255) NOT NULL UNIQUE,  -- Claude CLI tool use ID
    tool_name VARCHAR(255) NOT NULL,            -- Name of tool (e.g., "Edit", "Bash")
    tool_input JSONB NOT NULL,                  -- Tool input parameters
    status approval_status NOT NULL DEFAULT 'pending',
    approved_by UUID REFERENCES users(id),      -- User who approved/denied
    denied_reason TEXT,                         -- Optional reason for denial
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ                    -- When user responded
);

-- Indexes for performance
CREATE INDEX idx_tool_approvals_attempt_id ON tool_approvals(attempt_id);
CREATE INDEX idx_tool_approvals_status ON tool_approvals(status);
CREATE INDEX idx_tool_approvals_tool_use_id ON tool_approvals(tool_use_id);
CREATE INDEX idx_tool_approvals_created_at ON tool_approvals(created_at DESC);

-- Update agent_logs table to support structured JSON format (SDK mode)
ALTER TABLE agent_logs
    ADD COLUMN IF NOT EXISTS format_version INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS parsed_data JSONB DEFAULT NULL;

COMMENT ON COLUMN agent_logs.format_version IS '1=plain text (legacy --print mode), 2=stream-json (SDK mode)';
COMMENT ON COLUMN agent_logs.parsed_data IS 'Structured data for SDK mode logs (tool calls, results, control messages)';

-- Indexes for JSON log queries
CREATE INDEX IF NOT EXISTS idx_agent_logs_format_version ON agent_logs(format_version);
CREATE INDEX IF NOT EXISTS idx_agent_logs_parsed_data_gin ON agent_logs USING GIN (parsed_data);
