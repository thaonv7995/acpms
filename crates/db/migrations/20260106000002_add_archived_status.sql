-- Add 'archived' value to task_status enum
ALTER TYPE task_status ADD VALUE IF NOT EXISTS 'archived' AFTER 'done';
