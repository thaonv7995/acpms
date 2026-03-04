-- Fix seed admin user global_roles
-- Created: 2026-03-01
-- Purpose: Ensure admin@acpms.local has admin role (fixes bug where seed didn't set global_roles)
-- Affects: Databases where seed ran before the fix - admin user had default viewer role

UPDATE users
SET global_roles = ARRAY['admin']::system_role[],
    updated_at = NOW()
WHERE email = 'admin@acpms.local'
  AND NOT (global_roles @> ARRAY['admin']::system_role[]);
