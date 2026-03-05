-- Keep requirement status in sync with statuses of linked tasks.
-- Rules:
-- - No linked tasks => todo
-- - All linked tasks done/archived => done
-- - Otherwise, if any task has started (in_progress/in_review/blocked/done/archived) => in_progress
-- - Else => todo

CREATE OR REPLACE FUNCTION refresh_requirement_status_from_tasks(p_requirement_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF p_requirement_id IS NULL THEN
        RETURN;
    END IF;

    UPDATE requirements r
    SET
        status = CASE
            WHEN (
                SELECT COUNT(*)
                FROM tasks t
                WHERE t.requirement_id = r.id
            ) = 0
            THEN 'todo'::requirement_status

            WHEN (
                SELECT COUNT(*)
                FROM tasks t
                WHERE t.requirement_id = r.id
                  AND t.status IN ('done', 'archived')
            ) = (
                SELECT COUNT(*)
                FROM tasks t
                WHERE t.requirement_id = r.id
            )
            THEN 'done'::requirement_status

            WHEN (
                SELECT COUNT(*)
                FROM tasks t
                WHERE t.requirement_id = r.id
                  AND t.status IN ('in_progress', 'in_review', 'blocked', 'done', 'archived')
            ) > 0
            THEN 'in_progress'::requirement_status

            ELSE 'todo'::requirement_status
        END,
        updated_at = NOW()
    WHERE r.id = p_requirement_id;
END;
$$;

CREATE OR REPLACE FUNCTION trigger_refresh_requirement_status_from_tasks()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM refresh_requirement_status_from_tasks(NEW.requirement_id);
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.requirement_id IS DISTINCT FROM OLD.requirement_id THEN
            PERFORM refresh_requirement_status_from_tasks(OLD.requirement_id);
            PERFORM refresh_requirement_status_from_tasks(NEW.requirement_id);
        ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
            PERFORM refresh_requirement_status_from_tasks(NEW.requirement_id);
        END IF;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        PERFORM refresh_requirement_status_from_tasks(OLD.requirement_id);
        RETURN OLD;
    END IF;

    RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS trg_refresh_requirement_status_from_tasks ON tasks;
CREATE TRIGGER trg_refresh_requirement_status_from_tasks
AFTER INSERT OR UPDATE OF requirement_id, status OR DELETE
ON tasks
FOR EACH ROW
EXECUTE FUNCTION trigger_refresh_requirement_status_from_tasks();

-- Backfill all existing requirements once when migration is applied.
UPDATE requirements r
SET
    status = CASE
        WHEN stats.total = 0 THEN 'todo'::requirement_status
        WHEN stats.done_count = stats.total THEN 'done'::requirement_status
        WHEN stats.started_count > 0 THEN 'in_progress'::requirement_status
        ELSE 'todo'::requirement_status
    END,
    updated_at = NOW()
FROM (
    SELECT
        req.id AS requirement_id,
        COUNT(t.id)::INT AS total,
        COUNT(*) FILTER (WHERE t.status IN ('done', 'archived'))::INT AS done_count,
        COUNT(*) FILTER (WHERE t.status IN ('in_progress', 'in_review', 'blocked', 'done', 'archived'))::INT AS started_count
    FROM requirements req
    LEFT JOIN tasks t ON t.requirement_id = req.id
    GROUP BY req.id
) AS stats
WHERE r.id = stats.requirement_id;
