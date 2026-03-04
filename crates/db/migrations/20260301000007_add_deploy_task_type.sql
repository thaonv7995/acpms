-- Add 'deploy' to task_type enum for deployment tasks
ALTER TYPE task_type ADD VALUE IF NOT EXISTS 'deploy';

COMMENT ON TYPE task_type IS 'Task types: feature, bug, refactor, docs, test, init, hotfix, chore, spike, small_task, deploy';
