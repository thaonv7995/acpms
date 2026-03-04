-- Add metadata to projects table
-- Created: 2026-01-07

ALTER TABLE projects ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}';
