-- Migration: Audit Log Triggers for Sensitive Operations
-- Date: 2026-01-06
-- Purpose: Automatically log sensitive database operations to audit_logs table

-- ============================================================================
-- Helper Functions
-- ============================================================================

CREATE OR REPLACE FUNCTION get_request_ip()
RETURNS TEXT AS $$
BEGIN
    RETURN current_setting('app.request_ip', TRUE);
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION get_current_user_id()
RETURNS UUID AS $$
BEGIN
    RETURN current_setting('app.user_id', TRUE)::UUID;
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL;
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================================================
-- Generic Audit Trigger Function
-- ============================================================================

CREATE OR REPLACE FUNCTION audit_trigger_func()
RETURNS TRIGGER AS $$
DECLARE
    user_id_val UUID;
    ip_addr TEXT;
    action_name TEXT;
    old_data JSONB;
    new_data JSONB;
BEGIN
    user_id_val := get_current_user_id();
    ip_addr := get_request_ip();

    IF (TG_OP = 'INSERT') THEN
        action_name := TG_TABLE_NAME || '.create';
        new_data := to_jsonb(NEW);
        old_data := NULL;
    ELSIF (TG_OP = 'UPDATE') THEN
        action_name := TG_TABLE_NAME || '.update';
        new_data := to_jsonb(NEW);
        old_data := to_jsonb(OLD);
    ELSIF (TG_OP = 'DELETE') THEN
        action_name := TG_TABLE_NAME || '.delete';
        new_data := NULL;
        old_data := to_jsonb(OLD);
    END IF;

    INSERT INTO audit_logs (user_id, action, resource_type, resource_id, metadata)
    VALUES (
        user_id_val,
        action_name,
        TG_TABLE_NAME,
        COALESCE(NEW.id, OLD.id),
        jsonb_build_object(
            'operation', TG_OP,
            'old_data', old_data,
            'new_data', new_data,
            'ip_address', ip_addr,
            'timestamp', NOW()
        )
    );

    IF (TG_OP = 'DELETE') THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- ============================================================================
-- Audit Triggers for Sensitive Tables
-- ============================================================================

-- Users
DROP TRIGGER IF EXISTS audit_users_trigger ON users;
CREATE TRIGGER audit_users_trigger
    AFTER INSERT OR UPDATE OR DELETE ON users
    FOR EACH ROW
    EXECUTE FUNCTION audit_trigger_func();

-- Projects
DROP TRIGGER IF EXISTS audit_projects_trigger ON projects;
CREATE TRIGGER audit_projects_trigger
    AFTER UPDATE OR DELETE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION audit_trigger_func();

-- Project Members
DROP TRIGGER IF EXISTS audit_project_members_trigger ON project_members;
CREATE TRIGGER audit_project_members_trigger
    AFTER INSERT OR UPDATE OR DELETE ON project_members
    FOR EACH ROW
    EXECUTE FUNCTION audit_trigger_func();

-- GitLab Configurations
DROP TRIGGER IF EXISTS audit_gitlab_configurations_trigger ON gitlab_configurations;
CREATE TRIGGER audit_gitlab_configurations_trigger
    AFTER INSERT OR UPDATE OR DELETE ON gitlab_configurations
    FOR EACH ROW
    EXECUTE FUNCTION audit_trigger_func();

-- Task Attempts (status changes only)
DROP TRIGGER IF EXISTS audit_task_attempts_trigger ON task_attempts;
CREATE TRIGGER audit_task_attempts_trigger
    AFTER UPDATE ON task_attempts
    FOR EACH ROW
    WHEN (OLD.status IS DISTINCT FROM NEW.status)
    EXECUTE FUNCTION audit_trigger_func();

-- ============================================================================
-- Security & Indexes
-- ============================================================================

REVOKE UPDATE, DELETE ON audit_logs FROM PUBLIC;

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id_created_at
ON audit_logs(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_resource
ON audit_logs(resource_type, resource_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created_at
ON audit_logs(action, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at
ON audit_logs(created_at);

COMMENT ON TABLE audit_logs IS
'Immutable audit trail. Records inserted via triggers only. UPDATE/DELETE prohibited.';
