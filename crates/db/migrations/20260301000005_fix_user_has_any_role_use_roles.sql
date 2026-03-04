-- Migration: Fix user_has_any_role to use 'roles' column
-- Date: 2026-03-01
-- Description: project_members has 'roles' (array), not 'role' (singular).
-- The function may have been created with old schema. Ensure it uses roles && p_roles.

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
