-- Extend task_type enum with additional planning/execution categories.
-- These values are additive and safe for existing data.

ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'hotfix';
ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'chore';
ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'spike';
ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'small_task';

COMMENT ON TYPE task_type IS 'Task types: feature, bug, refactor, docs, test, init, hotfix, chore, spike, small_task';
