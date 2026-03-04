-- Add architecture_config column to projects table
-- This stores the project's architecture diagram configuration as JSON
-- Structure: { "nodes": [...], "edges": [...] }

ALTER TABLE projects
ADD COLUMN architecture_config JSONB DEFAULT '{"nodes": [], "edges": []}';

-- Add comment for documentation
COMMENT ON COLUMN projects.architecture_config IS 'Architecture diagram configuration with nodes and edges for React Flow visualization';
