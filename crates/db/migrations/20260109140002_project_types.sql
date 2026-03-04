-- Add project type enum and templates table
-- Phase 2: Project Creation Flow
-- Created: 2026-01-09

-- Create project_type enum
DO $$ BEGIN
    CREATE TYPE project_type AS ENUM (
        'web',
        'mobile',
        'desktop',
        'extension',
        'api',
        'microservice'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add project_type column to projects table
ALTER TABLE projects ADD COLUMN IF NOT EXISTS project_type project_type NOT NULL DEFAULT 'web';

-- Create project_templates table
CREATE TABLE IF NOT EXISTS project_templates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    project_type project_type NOT NULL,
    repository_url TEXT NOT NULL,
    tech_stack JSONB DEFAULT '[]'::jsonb,
    default_settings JSONB DEFAULT '{}'::jsonb,
    is_official BOOLEAN DEFAULT false,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_projects_project_type ON projects(project_type);
CREATE INDEX IF NOT EXISTS idx_templates_type ON project_templates(project_type);
CREATE INDEX IF NOT EXISTS idx_templates_official ON project_templates(is_official);
CREATE INDEX IF NOT EXISTS idx_templates_created_by ON project_templates(created_by);

-- Trigger for updated_at
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_project_templates_updated_at') THEN
        CREATE TRIGGER update_project_templates_updated_at
        BEFORE UPDATE ON project_templates
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;

-- Add comments for documentation
COMMENT ON TYPE project_type IS 'Enum of supported project types: web (Next.js, Vite, SvelteKit), mobile (React Native, Flutter, Expo), desktop (Electron, Tauri), extension (Chrome, Firefox), api (FastAPI, Express, NestJS), microservice (Go, Rust, gRPC)';
COMMENT ON TABLE project_templates IS 'Reusable project templates for quick scaffolding with predefined tech stacks and settings';
COMMENT ON COLUMN project_templates.tech_stack IS 'Array of technologies used in this template, e.g., ["React", "TypeScript", "Vite"]';
COMMENT ON COLUMN project_templates.default_settings IS 'Default project settings to apply when creating from this template';
COMMENT ON COLUMN project_templates.is_official IS 'Whether this is an officially maintained template';
