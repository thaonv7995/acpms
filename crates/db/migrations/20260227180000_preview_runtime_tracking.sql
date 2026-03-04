-- Track Docker runtime metadata for preview environments and prevent duplicate active rows per attempt

ALTER TABLE cloudflare_tunnels
    ADD COLUMN IF NOT EXISTS docker_project_name TEXT,
    ADD COLUMN IF NOT EXISTS compose_file_path TEXT,
    ADD COLUMN IF NOT EXISTS worktree_path TEXT,
    ADD COLUMN IF NOT EXISTS last_error TEXT,
    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS stopped_at TIMESTAMPTZ;

CREATE UNIQUE INDEX IF NOT EXISTS idx_cloudflare_tunnels_attempt_active_unique
    ON cloudflare_tunnels(attempt_id)
    WHERE deleted_at IS NULL;

COMMENT ON COLUMN cloudflare_tunnels.docker_project_name IS 'Docker compose project name used for this preview runtime';
COMMENT ON COLUMN cloudflare_tunnels.compose_file_path IS 'Absolute path to generated docker-compose.preview.yml';
COMMENT ON COLUMN cloudflare_tunnels.worktree_path IS 'Resolved worktree path for the attempt at runtime startup';
COMMENT ON COLUMN cloudflare_tunnels.last_error IS 'Last runtime-level error message (docker/cloudflare/command failures)';
COMMENT ON COLUMN cloudflare_tunnels.started_at IS 'When preview runtime was successfully started';
COMMENT ON COLUMN cloudflare_tunnels.stopped_at IS 'When preview runtime was last stopped';
