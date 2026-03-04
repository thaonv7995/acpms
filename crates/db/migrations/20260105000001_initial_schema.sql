-- ACPMS Initial Schema Migration
-- Created: 2026-01-05
-- Idempotent: safe to run when schema already exists (e.g. DB was set up manually)

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Enums (PostgreSQL has no CREATE TYPE IF NOT EXISTS for enum)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'project_role') THEN
        CREATE TYPE project_role AS ENUM ('owner', 'admin', 'developer', 'viewer');
    END IF;
END$$;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'task_type') THEN
        CREATE TYPE task_type AS ENUM ('feature', 'bug', 'refactor', 'docs', 'test');
    END IF;
END$$;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'task_status') THEN
        CREATE TYPE task_status AS ENUM ('todo', 'in_progress', 'in_review', 'blocked', 'done');
    END IF;
END$$;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'attempt_status') THEN
        CREATE TYPE attempt_status AS ENUM ('queued', 'running', 'success', 'failed', 'cancelled');
    END IF;
END$$;

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    gitlab_id INTEGER UNIQUE,
    gitlab_username VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    repository_url TEXT,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Project members table
CREATE TABLE IF NOT EXISTS project_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role project_role NOT NULL DEFAULT 'viewer',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, user_id)
);

-- Tasks table
CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    task_type task_type NOT NULL,
    status task_status NOT NULL DEFAULT 'todo',
    assigned_to UUID REFERENCES users(id) ON DELETE SET NULL,
    parent_task_id UUID REFERENCES tasks(id) ON DELETE CASCADE,
    gitlab_issue_id INTEGER,
    metadata JSONB DEFAULT '{}',
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Task attempts (agent execution sessions)
CREATE TABLE IF NOT EXISTS task_attempts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    status attempt_status NOT NULL DEFAULT 'queued',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Execution processes (running agent processes)
CREATE TABLE IF NOT EXISTS execution_processes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    process_id INTEGER,
    worktree_path TEXT,
    branch_name VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Agent logs (real-time logs from agent execution)
CREATE TABLE IF NOT EXISTS agent_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    log_type VARCHAR(50) NOT NULL, -- stdout, stderr, system
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Audit logs (user actions)
CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(255) NOT NULL,
    resource_type VARCHAR(100),
    resource_id UUID,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Repos table (cloned repositories)
CREATE TABLE IF NOT EXISTS repos (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    git_url TEXT NOT NULL,
    default_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GitLab projects sync
CREATE TABLE IF NOT EXISTS gitlab_projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_project_id INTEGER NOT NULL UNIQUE,
    gitlab_project_path TEXT NOT NULL,
    webhook_url TEXT,
    webhook_secret TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GitLab webhooks log
CREATE TABLE IF NOT EXISTS gitlab_webhooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    gitlab_project_id UUID REFERENCES gitlab_projects(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    processed BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Preview deployments
CREATE TABLE IF NOT EXISTS preview_deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    tunnel_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    destroyed_at TIMESTAMPTZ
);

-- System settings (key-value store)
CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_project_members_project ON project_members(project_id);
CREATE INDEX IF NOT EXISTS idx_project_members_user ON project_members(user_id);
CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_task_id);
CREATE INDEX IF NOT EXISTS idx_task_attempts_task ON task_attempts(task_id);
CREATE INDEX IF NOT EXISTS idx_task_attempts_status ON task_attempts(status);
CREATE INDEX IF NOT EXISTS idx_agent_logs_attempt ON agent_logs(attempt_id);
CREATE INDEX IF NOT EXISTS idx_gitlab_projects_project ON gitlab_projects(project_id);
-- Index only if gitlab_webhooks has gitlab_project_id (legacy schema may differ)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'gitlab_webhooks' AND column_name = 'gitlab_project_id') THEN
        CREATE INDEX IF NOT EXISTS idx_gitlab_webhooks_gitlab_project ON gitlab_webhooks(gitlab_project_id);
    END IF;
END$$;
CREATE INDEX IF NOT EXISTS idx_preview_deployments_attempt ON preview_deployments(attempt_id);

-- Updated_at triggers
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS update_users_updated_at ON users;
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_projects_updated_at ON projects;
CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON projects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_project_members_updated_at ON project_members;
CREATE TRIGGER update_project_members_updated_at BEFORE UPDATE ON project_members
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_tasks_updated_at ON tasks;
CREATE TRIGGER update_tasks_updated_at BEFORE UPDATE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_repos_updated_at ON repos;
CREATE TRIGGER update_repos_updated_at BEFORE UPDATE ON repos
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_gitlab_projects_updated_at ON gitlab_projects;
CREATE TRIGGER update_gitlab_projects_updated_at BEFORE UPDATE ON gitlab_projects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
