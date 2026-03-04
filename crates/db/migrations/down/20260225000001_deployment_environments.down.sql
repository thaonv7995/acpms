-- Manual rollback script for deployment environments migration
-- Corresponds to: 20260225000001_deployment_environments.sql
-- NOTE: This script is NOT executed by sqlx migrate automatically.
-- Run manually only when rollback is required.

BEGIN;

-- Drop triggers first
DROP TRIGGER IF EXISTS update_deployment_releases_updated_at ON deployment_releases;
DROP TRIGGER IF EXISTS update_deployment_runs_updated_at ON deployment_runs;
DROP TRIGGER IF EXISTS update_deployment_environment_secrets_updated_at ON deployment_environment_secrets;
DROP TRIGGER IF EXISTS update_deployment_environments_updated_at ON deployment_environments;

-- Drop tables in reverse dependency order
DROP TABLE IF EXISTS deployment_timeline_events;
DROP TABLE IF EXISTS deployment_releases;
DROP TABLE IF EXISTS deployment_runs;
DROP TABLE IF EXISTS deployment_environment_secrets;
DROP TABLE IF EXISTS deployment_environments;

-- Drop enum types in reverse creation order
DROP TYPE IF EXISTS deployment_timeline_event_type;
DROP TYPE IF EXISTS deployment_timeline_step;
DROP TYPE IF EXISTS deployment_release_status;
DROP TYPE IF EXISTS deployment_source_type;
DROP TYPE IF EXISTS deployment_trigger_type;
DROP TYPE IF EXISTS deployment_run_status;
DROP TYPE IF EXISTS deployment_secret_type;
DROP TYPE IF EXISTS deployment_artifact_strategy;
DROP TYPE IF EXISTS deployment_runtime_type;
DROP TYPE IF EXISTS deployment_target_type;

COMMIT;
