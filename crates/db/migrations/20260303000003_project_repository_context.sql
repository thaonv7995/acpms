ALTER TABLE projects
ADD COLUMN IF NOT EXISTS repository_context JSONB NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN projects.repository_context IS
'Provider-specific repository access and topology metadata. Contains provider, access_mode, capability flags, upstream/fork URLs, verification status, and resolved repo identifiers.';

CREATE INDEX IF NOT EXISTS idx_projects_repository_context_access_mode
ON projects ((repository_context->>'access_mode'));

CREATE INDEX IF NOT EXISTS idx_projects_repository_context_provider
ON projects ((repository_context->>'provider'));
