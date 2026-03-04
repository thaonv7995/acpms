-- Migration: Add Product Owner, Business Analyst, and QA roles
-- Date: 2026-01-06
-- Description: Extend project_role enum to include PO, BA, QA for complete project management

-- Add new role values to the existing enum
ALTER TYPE project_role ADD VALUE IF NOT EXISTS 'product_owner';
ALTER TYPE project_role ADD VALUE IF NOT EXISTS 'business_analyst';
ALTER TYPE project_role ADD VALUE IF NOT EXISTS 'quality_assurance';

-- Note: The permission hierarchy is:
-- Owner > Admin > Product Owner / Developer > Business Analyst > Quality Assurance > Viewer
--
-- Permission breakdown:
-- - Owner: Full control (all permissions)
-- - Admin: Administrative tasks (manage project, members, sprints)
-- - Product Owner: Product decisions (manage sprints, requirements, tasks)
-- - Developer: Implementation (execute tasks, modify code)
-- - Business Analyst: Requirements analysis (create/modify requirements)
-- - Quality Assurance: Testing and quality assurance (modify task status for testing)
-- - Viewer: Read-only access
