-- Performance Indexes Migration
-- Created: 2026-01-06
-- Purpose: Add indexes for frequently queried columns and common query patterns

-- Projects table indexes
CREATE INDEX IF NOT EXISTS idx_projects_repository_url ON projects(repository_url);
CREATE INDEX IF NOT EXISTS idx_projects_created_at ON projects(created_at DESC);

-- Tasks table indexes
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned_to ON tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);
-- Composite index for common query: list tasks by project and status
CREATE INDEX IF NOT EXISTS idx_tasks_project_status ON tasks(project_id, status);

-- Task Attempts table indexes
CREATE INDEX IF NOT EXISTS idx_task_attempts_status ON task_attempts(status);
CREATE INDEX IF NOT EXISTS idx_task_attempts_task_id ON task_attempts(task_id);
-- Composite index for task history queries
CREATE INDEX IF NOT EXISTS idx_task_attempts_task_created ON task_attempts(task_id, created_at DESC);

-- Project Members table indexes
CREATE INDEX IF NOT EXISTS idx_project_members_user_id ON project_members(user_id);
CREATE INDEX IF NOT EXISTS idx_project_members_project_id ON project_members(project_id);
-- Composite index for user's projects queries
CREATE INDEX IF NOT EXISTS idx_project_members_user_project ON project_members(user_id, project_id);

-- GitLab Configurations table indexes
CREATE INDEX IF NOT EXISTS idx_gitlab_configs_project_id ON gitlab_configurations(project_id);

-- Merge Requests table indexes
CREATE INDEX IF NOT EXISTS idx_merge_requests_task_id ON merge_requests(task_id);
CREATE INDEX IF NOT EXISTS idx_merge_requests_status ON merge_requests(status);
CREATE INDEX IF NOT EXISTS idx_merge_requests_created_at ON merge_requests(created_at DESC);

-- Audit Logs table indexes (for monitoring and compliance)
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource_type ON audit_logs(resource_type);

-- Agent Logs table indexes (for debugging and monitoring)
CREATE INDEX IF NOT EXISTS idx_agent_logs_attempt_id ON agent_logs(attempt_id);
CREATE INDEX IF NOT EXISTS idx_agent_logs_timestamp ON agent_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_logs_log_type ON agent_logs(log_type);

-- Comment for maintenance
COMMENT ON INDEX idx_tasks_project_status IS 'Optimizes queries listing tasks by project and filtering by status';
COMMENT ON INDEX idx_task_attempts_task_created IS 'Optimizes queries fetching task attempt history ordered by time';
COMMENT ON INDEX idx_project_members_user_project IS 'Optimizes queries checking user membership in projects';
