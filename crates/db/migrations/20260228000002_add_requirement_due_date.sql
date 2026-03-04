-- Add due_date to requirements for prioritization
ALTER TABLE requirements ADD COLUMN IF NOT EXISTS due_date DATE NULL;

CREATE INDEX IF NOT EXISTS idx_requirements_due_date ON requirements(due_date) WHERE due_date IS NOT NULL;
