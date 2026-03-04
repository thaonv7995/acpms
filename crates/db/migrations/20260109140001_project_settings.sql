-- Add project settings JSONB column and migrate existing require_review
-- Phase 1: Project Settings Enhancement
-- Created: 2026-01-09

-- Add settings JSONB column to projects table
ALTER TABLE projects ADD COLUMN IF NOT EXISTS settings JSONB NOT NULL DEFAULT '{
  "require_review": true,
  "auto_deploy": false,
  "preview_enabled": true,
  "gitops_enabled": true,
  "auto_execute": false,
  "auto_execute_types": [],
  "auto_execute_priority": "normal",
  "auto_retry": false,
  "max_retries": 3,
  "retry_backoff": "exponential",
  "timeout_mins": 30,
  "max_concurrent": 3,
  "preview_ttl_days": 7,
  "auto_merge": false,
  "deploy_branch": "main",
  "notify_on_success": false,
  "notify_on_failure": true,
  "notify_on_review": true,
  "notify_channels": []
}'::jsonb;

-- Migrate existing require_review column to settings if it exists
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'projects' AND column_name = 'require_review'
    ) THEN
        UPDATE projects
        SET settings = settings || jsonb_build_object('require_review', require_review);
    END IF;
END $$;

-- Create GIN index for efficient JSONB queries
CREATE INDEX IF NOT EXISTS idx_projects_settings_gin ON projects USING GIN (settings);

-- Create index for common setting queries
CREATE INDEX IF NOT EXISTS idx_projects_settings_require_review
ON projects ((settings->>'require_review'));

CREATE INDEX IF NOT EXISTS idx_projects_settings_auto_deploy
ON projects ((settings->>'auto_deploy'));

-- Add comments for documentation
COMMENT ON COLUMN projects.settings IS 'Project-level settings controlling agent, deploy, review, and notification behavior. Schema includes: require_review, auto_deploy, preview_enabled, gitops_enabled, auto_execute, max_retries, timeout_mins, preview_ttl_days, auto_merge, deploy_branch, and notification preferences.';
