-- Add token_blacklist table for revoked JWT tokens
CREATE TABLE token_blacklist (
    jti TEXT PRIMARY KEY,  -- JWT ID claim (unique identifier for each access token)
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,  -- Original token expiration (for auto-cleanup)
    reason TEXT,  -- Reason for revocation (logout, admin revoke, security breach, etc.)
    revoked_by UUID REFERENCES users(id)  -- User who revoked (for admin revocations)
);

-- Indexes for performance
CREATE INDEX idx_blacklist_jti ON token_blacklist(jti);
CREATE INDEX idx_blacklist_user_id ON token_blacklist(user_id);
CREATE INDEX idx_blacklist_expires ON token_blacklist(expires_at);

COMMENT ON TABLE token_blacklist IS 'Blacklist for revoked JWT access tokens (prevents use before natural expiration)';
COMMENT ON COLUMN token_blacklist.jti IS 'JWT ID claim - unique identifier from the token';
COMMENT ON COLUMN token_blacklist.expires_at IS 'Original token expiration - used for automatic cleanup after token would have expired anyway';
COMMENT ON COLUMN token_blacklist.reason IS 'Reason for revocation for audit trail';
