-- Add require_review column to projects table
-- When true: Agent completes work -> in_review status -> User reviews diff -> Approve -> Commit/Push
-- When false: Agent completes work -> Auto commit/push -> Create MR -> Done

ALTER TABLE projects ADD COLUMN IF NOT EXISTS require_review BOOLEAN NOT NULL DEFAULT true;

COMMENT ON COLUMN projects.require_review IS 'If true, agent changes require human review before commit/push. If false, auto commit/push after agent completes.';
