-- Add refresh_tokens table for server-side token storage
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,  -- SHA-256 hash of the refresh token
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    user_agent TEXT,
    ip_address INET
);

-- Indexes for performance
CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);

COMMENT ON TABLE refresh_tokens IS 'Server-side storage for refresh tokens with rotation support';
COMMENT ON COLUMN refresh_tokens.token_hash IS 'SHA-256 hash of the refresh token for secure storage';
COMMENT ON COLUMN refresh_tokens.expires_at IS 'Token expiration timestamp (typically 7 days)';
COMMENT ON COLUMN refresh_tokens.last_used_at IS 'Last time this refresh token was used to generate new access token';
