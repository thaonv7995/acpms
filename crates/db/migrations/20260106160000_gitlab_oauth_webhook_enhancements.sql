-- GitLab OAuth tokens table
CREATE TABLE IF NOT EXISTS gitlab_oauth_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    access_token_encrypted TEXT NOT NULL, -- AES-256-GCM encrypted
    refresh_token_encrypted TEXT, -- AES-256-GCM encrypted (if available)
    token_type TEXT NOT NULL DEFAULT 'Bearer',
    expires_at TIMESTAMPTZ,
    scope TEXT NOT NULL,
    gitlab_user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, project_id)
);

CREATE INDEX IF NOT EXISTS idx_gitlab_oauth_tokens_user ON gitlab_oauth_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_gitlab_oauth_tokens_project ON gitlab_oauth_tokens(project_id);
CREATE INDEX IF NOT EXISTS idx_gitlab_oauth_tokens_expires ON gitlab_oauth_tokens(expires_at) WHERE expires_at IS NOT NULL;

DROP TRIGGER IF EXISTS update_gitlab_oauth_tokens_updated_at ON gitlab_oauth_tokens;
CREATE TRIGGER update_gitlab_oauth_tokens_updated_at BEFORE UPDATE ON gitlab_oauth_tokens
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Webhook events log for audit trail and retry
CREATE TABLE IF NOT EXISTS webhook_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL, -- GitLab event ID for deduplication
    event_type TEXT NOT NULL, -- push, merge_request, pipeline
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    last_error TEXT,
    last_attempt_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, event_id) -- Prevent duplicate processing
);

CREATE INDEX IF NOT EXISTS idx_webhook_events_project ON webhook_events(project_id);
CREATE INDEX IF NOT EXISTS idx_webhook_events_status ON webhook_events(status) WHERE status IN ('pending', 'failed');
CREATE INDEX IF NOT EXISTS idx_webhook_events_created ON webhook_events(created_at);
CREATE INDEX IF NOT EXISTS idx_webhook_events_type ON webhook_events(event_type);

DROP TRIGGER IF EXISTS update_webhook_events_updated_at ON webhook_events;
CREATE TRIGGER update_webhook_events_updated_at BEFORE UPDATE ON webhook_events
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- GitLab sync metadata for incremental sync
CREATE TABLE IF NOT EXISTS gitlab_sync_metadata (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_project_id BIGINT NOT NULL,
    last_sync_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_branch_sync_at TIMESTAMPTZ,
    last_mr_sync_at TIMESTAMPTZ,
    last_pipeline_sync_at TIMESTAMPTZ,
    sync_status TEXT NOT NULL DEFAULT 'idle', -- idle, syncing, error
    sync_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id)
);

CREATE INDEX IF NOT EXISTS idx_gitlab_sync_metadata_project ON gitlab_sync_metadata(project_id);
CREATE INDEX IF NOT EXISTS idx_gitlab_sync_metadata_status ON gitlab_sync_metadata(sync_status);

DROP TRIGGER IF EXISTS update_gitlab_sync_metadata_updated_at ON gitlab_sync_metadata;
CREATE TRIGGER update_gitlab_sync_metadata_updated_at BEFORE UPDATE ON gitlab_sync_metadata
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Notification system tables (optional - P2)
CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    type TEXT NOT NULL, -- task_assigned, mr_created, task_completed, mention
    related_entity_type TEXT, -- task, project, merge_request
    related_entity_id UUID,
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_id);
CREATE INDEX IF NOT EXISTS idx_notifications_user_unread ON notifications(user_id, is_read) WHERE is_read = FALSE;
CREATE INDEX IF NOT EXISTS idx_notifications_created ON notifications(created_at);

-- Notification preferences
CREATE TABLE IF NOT EXISTS notification_preferences (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    in_app_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    task_assigned BOOLEAN NOT NULL DEFAULT TRUE,
    task_completed BOOLEAN NOT NULL DEFAULT TRUE,
    mr_created BOOLEAN NOT NULL DEFAULT TRUE,
    mr_merged BOOLEAN NOT NULL DEFAULT TRUE,
    mentions BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id)
);

CREATE INDEX IF NOT EXISTS idx_notification_preferences_user ON notification_preferences(user_id);

DROP TRIGGER IF EXISTS update_notification_preferences_updated_at ON notification_preferences;
CREATE TRIGGER update_notification_preferences_updated_at BEFORE UPDATE ON notification_preferences
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
