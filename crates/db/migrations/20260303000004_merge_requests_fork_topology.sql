-- Persist source/target repository topology for GitHub PR and GitLab MR flows.
ALTER TABLE merge_requests
ADD COLUMN IF NOT EXISTS source_repository_url TEXT,
ADD COLUMN IF NOT EXISTS target_repository_url TEXT,
ADD COLUMN IF NOT EXISTS source_branch TEXT,
ADD COLUMN IF NOT EXISTS target_branch TEXT,
ADD COLUMN IF NOT EXISTS source_project_id BIGINT,
ADD COLUMN IF NOT EXISTS target_project_id BIGINT,
ADD COLUMN IF NOT EXISTS source_namespace TEXT,
ADD COLUMN IF NOT EXISTS target_namespace TEXT;

CREATE INDEX IF NOT EXISTS idx_merge_requests_target_project_id
ON merge_requests(target_project_id)
WHERE target_project_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_merge_requests_source_project_id
ON merge_requests(source_project_id)
WHERE source_project_id IS NOT NULL;
