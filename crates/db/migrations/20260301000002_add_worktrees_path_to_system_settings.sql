-- Add worktrees_path to system_settings (admin-editable)
-- When NULL, server falls back to env WORKTREES_PATH or './worktrees'
-- Requires server restart for change to take effect

ALTER TABLE system_settings
ADD COLUMN IF NOT EXISTS worktrees_path TEXT;

COMMENT ON COLUMN system_settings.worktrees_path IS 'Path where agent worktrees are stored. Overrides WORKTREES_PATH env. Restart server to apply.';
