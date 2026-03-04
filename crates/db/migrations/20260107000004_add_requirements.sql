-- Add requirements table and link to tasks
-- Created: 2026-01-07

-- Requirement Status Enum
DO $$ BEGIN
    CREATE TYPE requirement_status AS ENUM ('draft', 'reviewing', 'approved', 'rejected', 'implemented');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE requirement_priority AS ENUM ('low', 'medium', 'high', 'critical');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Requirements table
CREATE TABLE IF NOT EXISTS requirements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(500) NOT NULL,
    content TEXT NOT NULL, -- Detailed description/spec
    status requirement_status NOT NULL DEFAULT 'draft',
    priority requirement_priority NOT NULL DEFAULT 'medium',
    metadata JSONB DEFAULT '{}',
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Link tasks to requirements
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS requirement_id UUID REFERENCES requirements(id) ON DELETE SET NULL;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_requirements_project ON requirements(project_id);
CREATE INDEX IF NOT EXISTS idx_requirements_status ON requirements(status);
CREATE INDEX IF NOT EXISTS idx_requirements_created_at ON requirements(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_requirement ON tasks(requirement_id);

-- Trigger for updated_at
-- Trigger for updated_at
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_requirements_updated_at') THEN
        CREATE TRIGGER update_requirements_updated_at BEFORE UPDATE ON requirements
            FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;
