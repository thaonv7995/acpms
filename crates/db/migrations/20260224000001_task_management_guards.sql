-- Task management integrity guards:
-- 1) Enforce one active sprint per project
-- 2) Enforce one active (queued/running) attempt per task

-- Keep the newest active sprint per project and complete the rest before adding unique constraint.
WITH ranked_active_sprints AS (
    SELECT
        id,
        project_id,
        ROW_NUMBER() OVER (
            PARTITION BY project_id
            ORDER BY created_at DESC, id DESC
        ) AS rn
    FROM sprints
    WHERE status = 'active'
)
UPDATE sprints s
SET status = 'completed',
    updated_at = NOW()
FROM ranked_active_sprints ras
WHERE s.id = ras.id
  AND ras.rn > 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_sprints_single_active_per_project
ON sprints (project_id)
WHERE status = 'active';

-- Keep the newest running/queued attempt per task and cancel duplicates before adding unique constraint.
WITH ranked_active_attempts AS (
    SELECT
        id,
        task_id,
        status,
        ROW_NUMBER() OVER (
            PARTITION BY task_id
            ORDER BY
                CASE WHEN status = 'running' THEN 0 ELSE 1 END,
                created_at DESC,
                id DESC
        ) AS rn
    FROM task_attempts
    WHERE status IN ('queued', 'running')
)
UPDATE task_attempts ta
SET status = 'cancelled',
    completed_at = COALESCE(ta.completed_at, NOW()),
    error_message = COALESCE(
        ta.error_message,
        'Cancelled automatically: duplicate active attempt cleaned up'
    )
FROM ranked_active_attempts raa
WHERE ta.id = raa.id
  AND raa.rn > 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_attempts_single_active_per_task
ON task_attempts (task_id)
WHERE status IN ('queued', 'running');
