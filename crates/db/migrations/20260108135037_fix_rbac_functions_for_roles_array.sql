-- Migration: Fix RBAC functions for roles array
-- Date: 2026-01-08
-- Description: Update RBAC functions to use 'roles' column (array) instead of 'role' (singular)
-- After migration 20260107055345 renamed the column

-- Fix user_has_role function
CREATE OR REPLACE FUNCTION user_has_role(
    p_user_id UUID,
    p_project_id UUID,
    p_role project_role
) RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM project_members
        WHERE user_id = p_user_id
        AND project_id = p_project_id
        AND p_role = ANY(roles)
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Fix user_has_any_role function to use roles array
CREATE OR REPLACE FUNCTION user_has_any_role(
    p_user_id UUID,
    p_project_id UUID,
    p_roles project_role[]
) RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM project_members
        WHERE user_id = p_user_id
        AND project_id = p_project_id
        AND roles && p_roles
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Fix get_user_roles function
CREATE OR REPLACE FUNCTION get_user_roles(
    p_user_id UUID,
    p_project_id UUID
) RETURNS project_role[] AS $$
DECLARE
    result project_role[];
BEGIN
    SELECT roles INTO result
    FROM project_members
    WHERE user_id = p_user_id
    AND project_id = p_project_id;

    RETURN COALESCE(result, '{}');
END;
$$ LANGUAGE plpgsql STABLE;

-- Fix user_can_modify_task function (it calls user_has_any_role, but let's ensure it works)
CREATE OR REPLACE FUNCTION user_can_modify_task(
    p_user_id UUID,
    p_task_id UUID
) RETURNS BOOLEAN AS $$
DECLARE
    v_project_id UUID;
BEGIN
    -- Get project_id from task
    SELECT project_id INTO v_project_id
    FROM tasks
    WHERE id = p_task_id;

    -- Check if user is owner, admin, or developer
    RETURN user_has_any_role(
        p_user_id,
        v_project_id,
        ARRAY['owner', 'admin', 'developer']::project_role[]
    );
END;
$$ LANGUAGE plpgsql STABLE;
