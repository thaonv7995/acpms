-- Update system_role enum to match project roles
-- PostgreSQL doesn't allow removing enum values easily, so we recreate the enum

-- Step 1: Drop the default constraint first
ALTER TABLE users ALTER COLUMN global_roles DROP DEFAULT;

-- Step 2: Drop dependent function if exists
DROP FUNCTION IF EXISTS user_has_global_role(uuid, system_role[]);

-- Step 3: Create new enum with all roles
CREATE TYPE system_role_new AS ENUM (
    'admin',
    'product_owner',
    'business_analyst',
    'developer',
    'quality_assurance',
    'viewer'
);

-- Step 4: Update the column to use the new enum
-- Map old 'user' values to 'viewer', keep 'admin' as 'admin'
ALTER TABLE users
    ALTER COLUMN global_roles TYPE system_role_new[]
    USING (
        CASE
            WHEN 'admin' = ANY(global_roles::text[]) THEN ARRAY['admin']::system_role_new[]
            ELSE ARRAY['viewer']::system_role_new[]
        END
    );

-- Step 5: Drop old enum and rename new one
DROP TYPE system_role CASCADE;
ALTER TYPE system_role_new RENAME TO system_role;

-- Step 6: Set new default value
ALTER TABLE users ALTER COLUMN global_roles SET DEFAULT '{viewer}';

-- Step 7: Recreate the helper function with new type
CREATE OR REPLACE FUNCTION user_has_global_role(user_id uuid, required_roles system_role[])
RETURNS boolean AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1 FROM users u
        WHERE u.id = user_id
        AND u.global_roles && required_roles
    );
END;
$$ LANGUAGE plpgsql;
