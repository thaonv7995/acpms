CREATE TABLE IF NOT EXISTS task_contexts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title VARCHAR(255),
    content_type VARCHAR(64) NOT NULL DEFAULT 'text/markdown',
    raw_content TEXT NOT NULL DEFAULT '',
    source VARCHAR(32) NOT NULL DEFAULT 'user',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS task_context_attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_context_id UUID NOT NULL REFERENCES task_contexts(id) ON DELETE CASCADE,
    storage_key VARCHAR(512) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    content_type VARCHAR(255) NOT NULL,
    size_bytes BIGINT,
    checksum VARCHAR(128),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(task_context_id, storage_key)
);

CREATE INDEX IF NOT EXISTS idx_task_contexts_task_id
    ON task_contexts(task_id);

CREATE INDEX IF NOT EXISTS idx_task_contexts_task_sort
    ON task_contexts(task_id, sort_order, created_at);

CREATE INDEX IF NOT EXISTS idx_task_context_attachments_context_id
    ON task_context_attachments(task_context_id);

DROP TRIGGER IF EXISTS update_task_contexts_updated_at ON task_contexts;
CREATE TRIGGER update_task_contexts_updated_at
    BEFORE UPDATE ON task_contexts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
