-- System-level settings table (singleton pattern - max 1 row)
-- Stores global configuration like GitLab connection, Cloudflare, etc.

-- Drop old key-value schema if exists
DROP TABLE IF EXISTS system_settings CASCADE;

-- Create new structured schema
CREATE TABLE system_settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- GitLab Self-Hosted Configuration
    gitlab_url TEXT NOT NULL DEFAULT 'https://gitlab.com',
    gitlab_pat_encrypted TEXT, -- AES-256-GCM encrypted PAT
    gitlab_auto_sync BOOLEAN NOT NULL DEFAULT true,

    -- Cloudflare Configuration
    cloudflare_account_id TEXT,
    cloudflare_api_token_encrypted TEXT, -- AES-256-GCM encrypted
    cloudflare_zone_id TEXT,
    cloudflare_base_domain TEXT,

    -- Notification Preferences (global defaults)
    notifications_email_enabled BOOLEAN NOT NULL DEFAULT true,
    notifications_slack_enabled BOOLEAN NOT NULL DEFAULT false,
    notifications_slack_webhook_url TEXT,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Ensure singleton (only one row can exist)
CREATE UNIQUE INDEX idx_system_settings_singleton ON system_settings ((true));

-- Auto-update timestamp trigger
CREATE TRIGGER update_system_settings_updated_at
    BEFORE UPDATE ON system_settings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default row
INSERT INTO system_settings (gitlab_url, gitlab_auto_sync)
VALUES ('https://gitlab.com', true);

-- Add comments for documentation
COMMENT ON TABLE system_settings IS 'Global system configuration (singleton table - only one row allowed)';
COMMENT ON COLUMN system_settings.gitlab_pat_encrypted IS 'AES-256-GCM encrypted GitLab Personal Access Token';
COMMENT ON COLUMN system_settings.cloudflare_api_token_encrypted IS 'AES-256-GCM encrypted Cloudflare API token';
