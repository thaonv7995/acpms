-- Add sprint_id to requirements table
ALTER TABLE requirements ADD COLUMN IF NOT EXISTS sprint_id UUID REFERENCES sprints(id) ON DELETE SET NULL;

-- Create index
CREATE INDEX IF NOT EXISTS idx_requirements_sprint_id ON requirements(sprint_id);
