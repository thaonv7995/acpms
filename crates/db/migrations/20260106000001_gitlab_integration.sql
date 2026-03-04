-- Create gitlab_configurations table
CREATE TABLE IF NOT EXISTS gitlab_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_project_id BIGINT NOT NULL,
    base_url TEXT NOT NULL DEFAULT 'https://gitlab.com',
    pat_encrypted TEXT NOT NULL,
    webhook_secret TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id)
);

-- Create merge_requests table
CREATE TABLE IF NOT EXISTS merge_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    gitlab_mr_iid BIGINT NOT NULL,
    web_url TEXT NOT NULL,
    status TEXT NOT NULL, -- opened, merged, closed
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_gitlab_configurations_project_id ON gitlab_configurations(project_id);
CREATE INDEX IF NOT EXISTS idx_merge_requests_task_id ON merge_requests(task_id);
