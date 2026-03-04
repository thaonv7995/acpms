-- Drop old gitlab_webhooks table if it exists (from initial schema)
-- The new webhook_events table (from 20260106160000) handles webhook event logging
DROP TABLE IF EXISTS gitlab_webhooks CASCADE;

-- GitLab Webhooks configuration table
CREATE TABLE gitlab_webhooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_id BIGINT NOT NULL, -- The ID of the webhook in GitLab
    url TEXT NOT NULL,
    events TEXT[] NOT NULL, -- Array of event names: "push", "merge_request"
    secret_token TEXT NOT NULL, -- Secret used to validate X-Gitlab-Token
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_gitlab_webhooks_project ON gitlab_webhooks(project_id);

CREATE TRIGGER update_gitlab_webhooks_updated_at BEFORE UPDATE ON gitlab_webhooks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
