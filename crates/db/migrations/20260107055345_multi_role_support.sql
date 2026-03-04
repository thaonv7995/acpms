-- Migration: Multi-role support (global + per-project)
-- Date: 2026-01-07
-- Description: Allow users to have multiple roles globally and per-project
-- Note: Made idempotent with IF NOT EXISTS / DO $$ blocks

-- ============================================================================
-- PART 1: Global Roles for Users
-- ============================================================================

-- Create system_role enum for global roles (separate from project roles)
DO $$ BEGIN
    CREATE TYPE system_role AS ENUM ('admin', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add global_roles column to users table (array of system_role)
DO $$ BEGIN
    ALTER TABLE users ADD COLUMN global_roles system_role[] NOT NULL DEFAULT '{user}';
EXCEPTION
    WHEN duplicate_column THEN null;
END $$;

-- ============================================================================
-- PART 2: Multi-roles per Project
-- ============================================================================

-- Add new roles column (array) to project_members if not exists
DO $$ BEGIN
    ALTER TABLE project_members ADD COLUMN roles project_role[] NOT NULL DEFAULT '{viewer}';
EXCEPTION
    WHEN duplicate_column THEN null;
END $$;

-- Migrate existing single role to array (only if role column still exists)
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'project_members' AND column_name = 'role') THEN
        UPDATE project_members SET roles = ARRAY[role] WHERE roles = '{viewer}' OR roles IS NULL;
        ALTER TABLE project_members DROP COLUMN role;
    END IF;
END $$;

-- ============================================================================
-- PART 3: Helper Functions
-- ============================================================================

-- Function to check if user has ANY of the specified global roles
CREATE OR REPLACE FUNCTION user_has_global_role(user_id UUID, required_roles system_role[])
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM users u
        WHERE u.id = user_id
        AND u.global_roles && required_roles
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to check if user has ANY of the specified roles in a project
CREATE OR REPLACE FUNCTION user_has_project_role(p_user_id UUID, p_project_id UUID, required_roles project_role[])
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM project_members pm
        WHERE pm.user_id = p_user_id
        AND pm.project_id = p_project_id
        AND pm.roles && required_roles
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Function to get all roles a user has in a project
CREATE OR REPLACE FUNCTION get_user_project_roles(p_user_id UUID, p_project_id UUID)
RETURNS project_role[] AS $$
DECLARE
    result project_role[];
BEGIN
    SELECT roles INTO result
    FROM project_members
    WHERE user_id = p_user_id AND project_id = p_project_id;

    RETURN COALESCE(result, '{}');
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- PART 4: Indexes for Performance
-- ============================================================================

-- GIN index for array containment queries on global_roles
CREATE INDEX IF NOT EXISTS idx_users_global_roles ON users USING GIN (global_roles);

-- GIN index for array containment queries on project roles
CREATE INDEX IF NOT EXISTS idx_project_members_roles ON project_members USING GIN (roles);

-- ============================================================================
-- PART 5: Update existing admin user to have admin global role
-- ============================================================================

UPDATE users SET global_roles = '{admin}' WHERE email = 'admin@acpms.local' AND NOT (global_roles @> '{admin}');
