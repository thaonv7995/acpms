-- Migration: Extend user_can_modify_task to include PO, BA, QA
-- Date: 2026-03-01
-- Description: Permission matrix grants ModifyTask to product_owner, business_analyst, quality_assurance.
-- The DB function previously only checked owner, admin, developer.

CREATE OR REPLACE FUNCTION user_can_modify_task(
    p_user_id UUID,
    p_task_id UUID
) RETURNS BOOLEAN AS $$
DECLARE
    v_project_id UUID;
BEGIN
    SELECT project_id INTO v_project_id
    FROM tasks
    WHERE id = p_task_id;

    -- Check if user has ModifyTask permission: owner, admin, product_owner, developer, business_analyst, quality_assurance
    RETURN user_has_any_role(
        p_user_id,
        v_project_id,
        ARRAY['owner', 'admin', 'product_owner', 'developer', 'business_analyst', 'quality_assurance']::project_role[]
    );
END;
$$ LANGUAGE plpgsql STABLE;
