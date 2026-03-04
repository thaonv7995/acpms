-- Configurable deployment environments and deployment run tracking
-- Phase 1: Environment CRUD, validation, and connection checks

-- Enum types
DO $$ BEGIN
    CREATE TYPE deployment_target_type AS ENUM ('local', 'ssh_remote');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_runtime_type AS ENUM ('compose', 'systemd', 'raw_script');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_artifact_strategy AS ENUM ('git_pull', 'upload_bundle', 'build_artifact');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_secret_type AS ENUM ('ssh_private_key', 'ssh_password', 'api_token', 'known_hosts', 'env_file');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_run_status AS ENUM ('queued', 'running', 'success', 'failed', 'cancelled', 'rolling_back', 'rolled_back');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_trigger_type AS ENUM ('manual', 'auto', 'rollback', 'retry');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_source_type AS ENUM ('branch', 'commit', 'artifact', 'release');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_release_status AS ENUM ('active', 'superseded', 'failed', 'rolled_back');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_timeline_step AS ENUM ('precheck', 'connect', 'prepare', 'deploy', 'domain_config', 'healthcheck', 'finalize', 'rollback');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE deployment_timeline_event_type AS ENUM ('system', 'agent', 'command', 'warning', 'error');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Deployment environments
CREATE TABLE IF NOT EXISTS deployment_environments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    target_type deployment_target_type NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    is_default BOOLEAN NOT NULL DEFAULT false,
    runtime_type deployment_runtime_type NOT NULL DEFAULT 'raw_script',
    deploy_path TEXT NOT NULL,
    artifact_strategy deployment_artifact_strategy NOT NULL DEFAULT 'build_artifact',
    branch_policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    healthcheck_url TEXT,
    healthcheck_timeout_secs INTEGER NOT NULL DEFAULT 60,
    healthcheck_expected_status INTEGER NOT NULL DEFAULT 200,
    target_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    domain_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Secret storage for deployment environments
CREATE TABLE IF NOT EXISTS deployment_environment_secrets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    environment_id UUID NOT NULL REFERENCES deployment_environments(id) ON DELETE CASCADE,
    secret_type deployment_secret_type NOT NULL,
    ciphertext TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(environment_id, secret_type)
);

-- Deployment runs and lifecycle
CREATE TABLE IF NOT EXISTS deployment_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id UUID NOT NULL REFERENCES deployment_environments(id) ON DELETE CASCADE,
    status deployment_run_status NOT NULL DEFAULT 'queued',
    trigger_type deployment_trigger_type NOT NULL DEFAULT 'manual',
    triggered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    source_type deployment_source_type NOT NULL DEFAULT 'branch',
    source_ref TEXT,
    attempt_id UUID REFERENCES task_attempts(id) ON DELETE SET NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Release history per environment
CREATE TABLE IF NOT EXISTS deployment_releases (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id UUID NOT NULL REFERENCES deployment_environments(id) ON DELETE CASCADE,
    run_id UUID NOT NULL REFERENCES deployment_runs(id) ON DELETE CASCADE,
    version_label TEXT NOT NULL,
    artifact_ref TEXT,
    git_commit_sha TEXT,
    status deployment_release_status NOT NULL DEFAULT 'active',
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Timeline events for deployment run detail view
CREATE TABLE IF NOT EXISTS deployment_timeline_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    run_id UUID NOT NULL REFERENCES deployment_runs(id) ON DELETE CASCADE,
    step deployment_timeline_step NOT NULL,
    event_type deployment_timeline_event_type NOT NULL,
    message TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Constraints and indexes
CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_env_unique_name_per_project
ON deployment_environments (project_id, lower(name));

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_env_single_default_per_project
ON deployment_environments (project_id)
WHERE is_default;

CREATE INDEX IF NOT EXISTS idx_deployment_env_project
ON deployment_environments (project_id);

CREATE INDEX IF NOT EXISTS idx_deployment_env_target_type
ON deployment_environments (target_type);

CREATE INDEX IF NOT EXISTS idx_deployment_env_created_at
ON deployment_environments (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_deployment_secrets_environment
ON deployment_environment_secrets (environment_id);

CREATE INDEX IF NOT EXISTS idx_deployment_runs_project
ON deployment_runs (project_id);

CREATE INDEX IF NOT EXISTS idx_deployment_runs_environment
ON deployment_runs (environment_id);

CREATE INDEX IF NOT EXISTS idx_deployment_runs_status
ON deployment_runs (status);

CREATE INDEX IF NOT EXISTS idx_deployment_runs_created_at
ON deployment_runs (created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_runs_single_active_per_environment
ON deployment_runs (environment_id)
WHERE status IN ('queued', 'running', 'rolling_back');

CREATE INDEX IF NOT EXISTS idx_deployment_releases_environment
ON deployment_releases (environment_id);

CREATE INDEX IF NOT EXISTS idx_deployment_releases_status
ON deployment_releases (status);

CREATE INDEX IF NOT EXISTS idx_deployment_timeline_run
ON deployment_timeline_events (run_id, created_at);

-- updated_at triggers
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_deployment_environments_updated_at') THEN
        CREATE TRIGGER update_deployment_environments_updated_at
        BEFORE UPDATE ON deployment_environments
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_deployment_environment_secrets_updated_at') THEN
        CREATE TRIGGER update_deployment_environment_secrets_updated_at
        BEFORE UPDATE ON deployment_environment_secrets
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_deployment_runs_updated_at') THEN
        CREATE TRIGGER update_deployment_runs_updated_at
        BEFORE UPDATE ON deployment_runs
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_deployment_releases_updated_at') THEN
        CREATE TRIGGER update_deployment_releases_updated_at
        BEFORE UPDATE ON deployment_releases
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;
