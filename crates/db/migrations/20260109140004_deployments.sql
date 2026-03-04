-- Add build artifacts and deployment tables
-- Phase 5: Deployment & Publishing
-- Created: 2026-01-09

-- Create build_artifacts table
CREATE TABLE IF NOT EXISTS build_artifacts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    artifact_key TEXT NOT NULL,
    artifact_type VARCHAR(50) NOT NULL,
    size_bytes BIGINT,
    file_count INTEGER,
    build_command TEXT,
    build_duration_secs INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Drop old preview_deployments table if exists (created in earlier migration with different schema)
DROP TABLE IF EXISTS preview_deployments CASCADE;

-- Create preview_deployments table with full schema
CREATE TABLE preview_deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    artifact_id UUID REFERENCES build_artifacts(id) ON DELETE SET NULL,
    url TEXT NOT NULL,
    tunnel_id TEXT,
    dns_record_id TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ NOT NULL,
    destroyed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create production_deployments table
CREATE TABLE IF NOT EXISTS production_deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    artifact_id UUID REFERENCES build_artifacts(id) ON DELETE SET NULL,
    deployment_type VARCHAR(50) NOT NULL,
    url TEXT NOT NULL,
    deployment_id TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    triggered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_build_artifacts_attempt ON build_artifacts(attempt_id);
CREATE INDEX IF NOT EXISTS idx_build_artifacts_project ON build_artifacts(project_id);
CREATE INDEX IF NOT EXISTS idx_build_artifacts_type ON build_artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_build_artifacts_created_at ON build_artifacts(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_preview_deployments_attempt ON preview_deployments(attempt_id);
CREATE INDEX IF NOT EXISTS idx_preview_deployments_project ON preview_deployments(project_id);
CREATE INDEX IF NOT EXISTS idx_preview_deployments_status ON preview_deployments(status);
CREATE INDEX IF NOT EXISTS idx_preview_deployments_expires_at ON preview_deployments(expires_at);

CREATE INDEX IF NOT EXISTS idx_production_deployments_project ON production_deployments(project_id);
CREATE INDEX IF NOT EXISTS idx_production_deployments_status ON production_deployments(status);
CREATE INDEX IF NOT EXISTS idx_production_deployments_created_at ON production_deployments(created_at DESC);

-- Triggers for updated_at
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_preview_deployments_updated_at') THEN
        CREATE TRIGGER update_preview_deployments_updated_at
        BEFORE UPDATE ON preview_deployments
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_production_deployments_updated_at') THEN
        CREATE TRIGGER update_production_deployments_updated_at
        BEFORE UPDATE ON production_deployments
        FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;

-- Add comments for documentation
COMMENT ON TABLE build_artifacts IS 'Build output artifacts stored in MinIO with metadata';
COMMENT ON COLUMN build_artifacts.artifact_key IS 'Storage key in MinIO: builds/{project_id}/{attempt_id}/{artifact_name}';
COMMENT ON COLUMN build_artifacts.artifact_type IS 'Type of artifact: dist, binary, apk, ipa, image, etc.';
COMMENT ON COLUMN build_artifacts.build_command IS 'Command used to generate this artifact';
COMMENT ON COLUMN build_artifacts.build_duration_secs IS 'Time taken to build in seconds';

COMMENT ON TABLE preview_deployments IS 'Preview environments using Cloudflare tunnels with auto-expiry';
COMMENT ON COLUMN preview_deployments.url IS 'Public URL for preview environment, e.g., https://task-abc123.preview.domain';
COMMENT ON COLUMN preview_deployments.tunnel_id IS 'Cloudflare tunnel ID';
COMMENT ON COLUMN preview_deployments.dns_record_id IS 'Cloudflare DNS record ID for cleanup';
COMMENT ON COLUMN preview_deployments.status IS 'Deployment status: active, expired, destroyed';
COMMENT ON COLUMN preview_deployments.expires_at IS 'Auto-expiry timestamp based on preview_ttl_days setting';
COMMENT ON COLUMN preview_deployments.destroyed_at IS 'Timestamp when preview was destroyed';

COMMENT ON TABLE production_deployments IS 'Production deployments to Cloudflare Pages, Workers, or external services';
COMMENT ON COLUMN production_deployments.deployment_type IS 'Target platform: pages, workers, container, manual';
COMMENT ON COLUMN production_deployments.deployment_id IS 'External deployment ID from target platform';
COMMENT ON COLUMN production_deployments.status IS 'Deployment status: active, failed, superseded';
COMMENT ON COLUMN production_deployments.metadata IS 'Platform-specific deployment metadata';
