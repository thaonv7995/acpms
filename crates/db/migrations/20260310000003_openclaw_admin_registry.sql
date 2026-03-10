CREATE TABLE IF NOT EXISTS openclaw_clients (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    client_id TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'revoked')),
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NULL,
    last_seen_ip INET NULL,
    last_seen_user_agent TEXT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    disabled_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_openclaw_clients_status
    ON openclaw_clients (status, enrolled_at DESC);

DROP TRIGGER IF EXISTS update_openclaw_clients_updated_at ON openclaw_clients;
CREATE TRIGGER update_openclaw_clients_updated_at
    BEFORE UPDATE ON openclaw_clients
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE IF NOT EXISTS openclaw_client_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    client_id UUID NOT NULL REFERENCES openclaw_clients(id) ON DELETE CASCADE,
    key_id TEXT NOT NULL,
    algorithm TEXT NOT NULL DEFAULT 'ed25519',
    public_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'revoked')),
    last_used_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (client_id, key_id)
);

CREATE INDEX IF NOT EXISTS idx_openclaw_client_keys_client_status
    ON openclaw_client_keys (client_id, status);

DROP TRIGGER IF EXISTS update_openclaw_client_keys_updated_at ON openclaw_client_keys;
CREATE TRIGGER update_openclaw_client_keys_updated_at
    BEFORE UPDATE ON openclaw_client_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE IF NOT EXISTS openclaw_bootstrap_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_hash TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    suggested_display_name TEXT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'used', 'expired', 'revoked')),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    used_by_client_id UUID NULL REFERENCES openclaw_clients(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_openclaw_bootstrap_tokens_status_expiry
    ON openclaw_bootstrap_tokens (status, expires_at);

DROP TRIGGER IF EXISTS update_openclaw_bootstrap_tokens_updated_at ON openclaw_bootstrap_tokens;
CREATE TRIGGER update_openclaw_bootstrap_tokens_updated_at
    BEFORE UPDATE ON openclaw_bootstrap_tokens
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
