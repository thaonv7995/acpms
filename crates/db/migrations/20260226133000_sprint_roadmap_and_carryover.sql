-- Sprint roadmap + carry-over support

-- 1) Extend sprints table for roadmap/closure metadata
ALTER TABLE sprints
    ADD COLUMN IF NOT EXISTS sequence INTEGER,
    ADD COLUMN IF NOT EXISTS goal TEXT,
    ADD COLUMN IF NOT EXISTS closed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS closed_by UUID REFERENCES users(id) ON DELETE SET NULL;

-- Backfill sprint sequence per project by chronological order.
WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY project_id
            ORDER BY COALESCE(start_date, created_at) ASC, created_at ASC, id ASC
        ) AS seq
    FROM sprints
)
UPDATE sprints s
SET sequence = ranked.seq
FROM ranked
WHERE s.id = ranked.id
  AND s.sequence IS NULL;

-- Keep sequence mandatory once backfilled.
ALTER TABLE sprints
    ALTER COLUMN sequence SET NOT NULL;

-- Ensure sprint sequence uniqueness inside a project.
CREATE UNIQUE INDEX IF NOT EXISTS idx_sprints_project_sequence
ON sprints (project_id, sequence);

-- Helpful query index for project/sprint task stats.
CREATE INDEX IF NOT EXISTS idx_tasks_project_sprint_status
ON tasks (project_id, sprint_id, status);

-- 2) Audit trail for sprint carry-over task moves
CREATE TABLE IF NOT EXISTS sprint_task_movements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_sprint_id UUID REFERENCES sprints(id) ON DELETE SET NULL,
    to_sprint_id UUID REFERENCES sprints(id) ON DELETE SET NULL,
    moved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    moved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_sprint_task_movements_project_time
ON sprint_task_movements (project_id, moved_at DESC);

CREATE INDEX IF NOT EXISTS idx_sprint_task_movements_task
ON sprint_task_movements (task_id, moved_at DESC);

CREATE INDEX IF NOT EXISTS idx_sprint_task_movements_from_to
ON sprint_task_movements (from_sprint_id, to_sprint_id);
