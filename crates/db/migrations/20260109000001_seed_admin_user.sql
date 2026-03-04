-- Seed Admin User
-- Created: 2026-01-09
-- Purpose: Create default admin user for initial setup
-- Email: admin@acpms.local
-- Password: admin123 (CHANGE THIS IN PRODUCTION!)

-- Insert admin user with system admin role
-- Password hash for "admin123" using bcrypt with cost 12
INSERT INTO users (
    email,
    name,
    password_hash,
    global_roles,
    created_at,
    updated_at
)
VALUES (
    'admin@acpms.local',
    'Admin User',
    '$2y$12$ovlS6fjllYtHTCmNjNANPegmUp96x.67NXlc.cPoWTcEurDB4rbJK', -- admin123
    ARRAY['admin']::system_role[],
    NOW(),
    NOW()
)
ON CONFLICT (email) DO UPDATE SET
    global_roles = ARRAY['admin']::system_role[],
    updated_at = NOW();

COMMENT ON TABLE users IS 'Users table with seeded admin account (admin@acpms.local / admin123)';
