-- RBAC Helper Functions
-- Created: 2026-01-05

-- Check if user has a specific role in project
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
        AND role = p_role
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Check if user has any of the specified roles
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
        AND role = ANY(p_roles)
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- Get user's roles in project
CREATE OR REPLACE FUNCTION get_user_roles(
    p_user_id UUID,
    p_project_id UUID
) RETURNS project_role[] AS $$
DECLARE
    roles project_role[];
BEGIN
    SELECT ARRAY_AGG(role) INTO roles
    FROM project_members
    WHERE user_id = p_user_id
    AND project_id = p_project_id;

    RETURN COALESCE(roles, '{}');
END;
$$ LANGUAGE plpgsql STABLE;

-- Check if user can perform action on task
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
