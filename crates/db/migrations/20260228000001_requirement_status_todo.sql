-- Requirement Status: draft/reviewing/approved/rejected/implemented → todo/in_progress/done
-- Created: 2026-02-28
-- See: docs/todo-feat/IMPLEMENTATION_GUIDE.md

-- Step 1: Create new enum
CREATE TYPE requirement_status_new AS ENUM ('todo', 'in_progress', 'done');

-- Step 2: Drop default, alter column type with USING
ALTER TABLE requirements ALTER COLUMN status DROP DEFAULT;
ALTER TABLE requirements ALTER COLUMN status TYPE requirement_status_new
  USING (
    CASE status::text
      WHEN 'draft' THEN 'todo'::requirement_status_new
      WHEN 'reviewing' THEN 'in_progress'::requirement_status_new
      WHEN 'approved' THEN 'in_progress'::requirement_status_new
      WHEN 'rejected' THEN 'done'::requirement_status_new
      WHEN 'implemented' THEN 'done'::requirement_status_new
      ELSE 'todo'::requirement_status_new
    END
  );

-- Step 3: Set new default
ALTER TABLE requirements ALTER COLUMN status SET DEFAULT 'todo'::requirement_status_new;

-- Step 4: Drop old enum, rename new
DROP TYPE requirement_status;
ALTER TYPE requirement_status_new RENAME TO requirement_status;
