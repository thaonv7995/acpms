-- Fix inconsistent state: task status='todo' but has attempt status='running'.
-- When attempt runs, task must be in_progress. This repairs existing bad data.

UPDATE tasks t
SET status = 'in_progress', updated_at = NOW()
FROM task_attempts ta
WHERE ta.task_id = t.id
  AND ta.status = 'running'
  AND t.status = 'todo';
