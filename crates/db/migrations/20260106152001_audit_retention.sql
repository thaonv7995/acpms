-- Migration: Audit Log Retention Policy
-- Date: 2026-01-06
-- Purpose: Cleanup function for old audit logs (90 days retention)

CREATE OR REPLACE FUNCTION cleanup_old_audit_logs()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM audit_logs
    WHERE created_at < NOW() - INTERVAL '90 days';

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_old_audit_logs() IS
'Deletes audit logs older than 90 days. Should be run daily via cron job.';

-- Optional: Schedule via pg_cron (if installed)
-- SELECT cron.schedule('cleanup-audit-logs', '0 2 * * *', 'SELECT cleanup_old_audit_logs()');
