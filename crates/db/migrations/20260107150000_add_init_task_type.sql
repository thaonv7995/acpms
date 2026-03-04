-- Add 'init' to task_type enum
-- This supports project initialization flows

ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'init';

-- Add comment for documentation
COMMENT ON TYPE task_type IS 'Task types: feature, bug, refactor, docs, test, init';
