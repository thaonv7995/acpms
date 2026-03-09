CREATE TABLE IF NOT EXISTS project_documents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    document_kind VARCHAR(32) NOT NULL DEFAULT 'other'
        CHECK (document_kind IN (
            'architecture',
            'api_spec',
            'database_schema',
            'business_rules',
            'runbook',
            'notes',
            'other'
        )),
    content_type VARCHAR(255) NOT NULL,
    storage_key VARCHAR(512) NOT NULL,
    checksum VARCHAR(128),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    source VARCHAR(32) NOT NULL
        CHECK (source IN ('upload', 'repo_sync', 'api')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
    ingestion_status VARCHAR(32) NOT NULL DEFAULT 'pending'
        CHECK (ingestion_status IN ('pending', 'indexing', 'indexed', 'failed')),
    index_error TEXT,
    indexed_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, filename)
);

CREATE TABLE IF NOT EXISTS project_document_chunks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    document_id UUID NOT NULL REFERENCES project_documents(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    content_hash VARCHAR(128) NOT NULL,
    token_count INTEGER,
    embedding REAL[] NOT NULL DEFAULT '{}'::REAL[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(document_id, chunk_index)
);

CREATE INDEX IF NOT EXISTS idx_project_documents_project_id
    ON project_documents(project_id);

CREATE INDEX IF NOT EXISTS idx_project_documents_project_updated
    ON project_documents(project_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_project_documents_project_title
    ON project_documents(project_id, title);

CREATE INDEX IF NOT EXISTS idx_project_document_chunks_project_id
    ON project_document_chunks(project_id);

CREATE INDEX IF NOT EXISTS idx_project_document_chunks_document_id
    ON project_document_chunks(document_id);

DROP TRIGGER IF EXISTS update_project_documents_updated_at ON project_documents;
CREATE TRIGGER update_project_documents_updated_at
    BEFORE UPDATE ON project_documents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
