-- Add GitHub PR support to merge_requests
-- provider: 'gitlab' | 'github'
-- github_pr_number: PR number when provider='github', NULL for GitLab
-- gitlab_mr_iid: nullable for GitHub (was NOT NULL, now nullable for new GitHub rows)
ALTER TABLE merge_requests
ADD COLUMN IF NOT EXISTS provider TEXT NOT NULL DEFAULT 'gitlab',
ADD COLUMN IF NOT EXISTS github_pr_number BIGINT;

-- Allow gitlab_mr_iid to be NULL for GitHub PRs (GitLab rows keep value)
ALTER TABLE merge_requests
ALTER COLUMN gitlab_mr_iid DROP NOT NULL;

-- Constraint: either gitlab_mr_iid (GitLab) or github_pr_number (GitHub) must be set
-- We enforce in application logic; DB allows both for flexibility during migration

CREATE INDEX IF NOT EXISTS idx_merge_requests_provider ON merge_requests(provider);
CREATE INDEX IF NOT EXISTS idx_merge_requests_github_pr ON merge_requests(github_pr_number) WHERE github_pr_number IS NOT NULL;
