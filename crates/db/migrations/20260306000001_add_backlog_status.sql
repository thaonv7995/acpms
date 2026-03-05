-- Add 'backlog' value to task_status enum for explicit backlog workflow.
ALTER TYPE task_status ADD VALUE IF NOT EXISTS 'backlog' BEFORE 'todo';
