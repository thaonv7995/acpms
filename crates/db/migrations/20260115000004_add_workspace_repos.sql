-- Migration: Add workspace_repos for multi-repository support
-- Purpose: Support tasks that span multiple git repositories
-- Phase 4 of vibe-kanban alignment

CREATE TABLE workspace_repos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,

    -- Repo identification
    repo_name TEXT NOT NULL,
    repo_url TEXT NOT NULL,

    -- Workspace paths
    worktree_path TEXT NOT NULL,
    relative_path TEXT NOT NULL DEFAULT '.',

    -- Branch management
    target_branch TEXT NOT NULL,
    base_branch TEXT NOT NULL DEFAULT 'main',

    -- Primary repo designation
    is_primary BOOLEAN NOT NULL DEFAULT false,

    -- Metadata (commit SHAs, MR URLs, etc.)
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Constraints
    UNIQUE(attempt_id, repo_name)
);

-- Indexes for performance
CREATE INDEX idx_workspace_repos_attempt ON workspace_repos(attempt_id);
CREATE INDEX idx_workspace_repos_project ON workspace_repos(project_id);
CREATE INDEX idx_workspace_repos_primary ON workspace_repos(attempt_id, is_primary);

-- Ensure only one primary repo per attempt
CREATE UNIQUE INDEX idx_workspace_repos_single_primary
ON workspace_repos(attempt_id)
WHERE is_primary = true;

-- Comments
COMMENT ON TABLE workspace_repos IS 'Links task attempts to multiple git repositories for multi-repo workspaces';
COMMENT ON COLUMN workspace_repos.relative_path IS 'Relative path in workspace (e.g., backend/, frontend/)';
COMMENT ON COLUMN workspace_repos.is_primary IS 'Primary repository is the main focus for agent execution';
