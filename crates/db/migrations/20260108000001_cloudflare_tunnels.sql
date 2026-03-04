-- Create cloudflare_tunnels table for preview environment management
CREATE TABLE cloudflare_tunnels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attempt_id UUID NOT NULL REFERENCES task_attempts(id) ON DELETE CASCADE,
    tunnel_id VARCHAR(255) NOT NULL,
    tunnel_name VARCHAR(255) NOT NULL,
    -- Cloudflare credentials encrypted with AES-256-GCM
    credentials_encrypted TEXT NOT NULL,
    preview_url TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'creating',
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    -- Constraints
    UNIQUE(tunnel_id),
    CONSTRAINT valid_status CHECK (status IN ('creating', 'active', 'failed', 'deleted'))
);

-- Index for looking up tunnels by attempt
CREATE INDEX idx_cloudflare_tunnels_attempt ON cloudflare_tunnels(attempt_id);

-- Index for cleanup job (find active/expired tunnels)
CREATE INDEX idx_cloudflare_tunnels_status ON cloudflare_tunnels(status, expires_at)
    WHERE deleted_at IS NULL;

-- Add preview_url column to task_attempts table
ALTER TABLE task_attempts
    ADD COLUMN preview_url TEXT;

-- Add index for fast lookup of attempts with previews
CREATE INDEX idx_task_attempts_preview_url ON task_attempts(preview_url)
    WHERE preview_url IS NOT NULL;

COMMENT ON TABLE cloudflare_tunnels IS 'Cloudflare Tunnel configurations for preview environments';
COMMENT ON COLUMN cloudflare_tunnels.credentials_encrypted IS 'AES-256-GCM encrypted JSON containing tunnel credentials';
COMMENT ON COLUMN cloudflare_tunnels.expires_at IS 'When the tunnel should be automatically cleaned up (default 7 days)';
COMMENT ON COLUMN cloudflare_tunnels.deleted_at IS 'Soft delete timestamp for audit trail';
