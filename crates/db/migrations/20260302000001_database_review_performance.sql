-- Database Review Performance Fixes
-- Addresses: execution_processes hot path, MR query, task board, duplicate indexes, search

-- =============================================================================
-- [High] execution_processes: index for (attempt_id + created_at) access path
-- Used by: fetch_attempt_execution_processes, list_execution_processes, WS streams
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_execution_processes_attempt_created
ON execution_processes(attempt_id, created_at ASC);

COMMENT ON INDEX idx_execution_processes_attempt_created IS
'Hot path: endpoints/streams query by attempt_id ORDER BY created_at';

-- =============================================================================
-- [Medium] pg_trgm for ILIKE/LIKE search (projects.name, MR search)
-- Enables: CREATE INDEX USING gin(col gin_trgm_ops) for prefix/suffix search
-- =============================================================================
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Projects search: p.name ILIKE '%' || $x || '%'
CREATE INDEX IF NOT EXISTS idx_projects_name_trgm
ON projects USING gin(name gin_trgm_ops);

-- =============================================================================
-- [Medium] Drop redundant indexes (duplicate function, different names)
-- Reduces write amplification and planner overhead
-- =============================================================================
-- task_attempts: idx_task_attempts_task (initial) is prefix of idx_task_attempts_task_id
-- Both index task_id. Keep idx_task_attempts_task_id (more descriptive).
DROP INDEX IF EXISTS idx_task_attempts_task;

-- project_members: idx_project_members_project vs idx_project_members_project_id (same column)
DROP INDEX IF EXISTS idx_project_members_project;
-- idx_project_members_project_id from 20260106154000 covers same

-- project_members: idx_project_members_user vs idx_project_members_user_id
DROP INDEX IF EXISTS idx_project_members_user;

-- tasks: idx_tasks_project vs idx_tasks_project_id
DROP INDEX IF EXISTS idx_tasks_project;

-- tasks: idx_tasks_assigned vs idx_tasks_assigned_to
DROP INDEX IF EXISTS idx_tasks_assigned;

-- merge_requests: idx_merge_requests_task_id may exist twice (20260106000001, 20260106154000)
-- DROP would remove both; CREATE IF NOT EXISTS in migration would recreate. Skip to avoid breaking.
