use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};
use std::sync::Arc;

use acpms_services::OpenClawGatewayMetricsObserver;

/// Application metrics for Prometheus monitoring
#[derive(Clone)]
pub struct Metrics {
    #[allow(dead_code)]
    registry: Arc<Registry>,

    // HTTP metrics
    #[allow(dead_code)]
    pub http_requests_total: IntCounterVec,
    #[allow(dead_code)]
    pub http_request_duration_seconds: HistogramVec,
    #[allow(dead_code)]
    pub http_requests_in_flight: IntGauge,

    // Database metrics
    #[allow(dead_code)]
    pub db_connections_active: IntGauge,
    #[allow(dead_code)]
    pub db_connections_idle: IntGauge,
    #[allow(dead_code)]
    pub db_query_duration_seconds: HistogramVec,

    // Worker metrics
    #[allow(dead_code)]
    pub worker_queue_depth: IntGauge,
    #[allow(dead_code)]
    pub worker_active_jobs: IntGauge,
    #[allow(dead_code)]
    pub worker_jobs_total: IntCounterVec,

    // Business metrics
    #[allow(dead_code)]
    pub tasks_created_total: IntCounterVec,
    #[allow(dead_code)]
    pub task_attempts_total: IntCounterVec,
    #[allow(dead_code)]
    pub projects_total: IntGauge,

    // Deployment metrics
    #[allow(dead_code)]
    pub deployment_runs_total: IntCounterVec,
    #[allow(dead_code)]
    pub deployment_run_duration_seconds: HistogramVec,
    #[allow(dead_code)]
    pub deployment_failures_total: IntCounterVec,
    #[allow(dead_code)]
    pub rollback_runs_total: IntCounterVec,

    // Agent auth metrics
    #[allow(dead_code)]
    pub auth_session_started_total: IntCounterVec,
    #[allow(dead_code)]
    pub auth_session_success_total: IntCounterVec,
    #[allow(dead_code)]
    pub auth_session_failed_total: IntCounterVec,
    #[allow(dead_code)]
    pub auth_session_timeout_total: IntCounterVec,

    // Repository access and fork flow metrics
    #[allow(dead_code)]
    pub repository_access_evaluations_total: IntCounterVec,
    #[allow(dead_code)]
    pub repository_fork_operations_total: IntCounterVec,
    #[allow(dead_code)]
    pub repository_backfill_total: IntCounterVec,
    #[allow(dead_code)]
    pub repository_attempt_blocks_total: IntCounterVec,

    // OpenClaw gateway metrics
    #[allow(dead_code)]
    pub openclaw_gateway_auth_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_event_stream_connections_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_event_stream_active_connections: IntGauge,
    #[allow(dead_code)]
    pub openclaw_event_stream_replay_events_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_event_stream_cursor_expired_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_events_recorded_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_webhook_deliveries_total: IntCounterVec,
    #[allow(dead_code)]
    pub openclaw_webhook_duration_seconds: HistogramVec,
}

impl Metrics {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Arc::new(Registry::new());

        // HTTP metrics
        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests").namespace("acpms"),
            &["method", "path", "status"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .namespace("acpms")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["method", "path"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;

        let http_requests_in_flight = IntGauge::new(
            "acpms_http_requests_in_flight",
            "Number of HTTP requests currently being processed",
        )?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;

        // Database metrics
        let db_connections_active = IntGauge::new(
            "acpms_db_connections_active",
            "Number of active database connections",
        )?;
        registry.register(Box::new(db_connections_active.clone()))?;

        let db_connections_idle = IntGauge::new(
            "acpms_db_connections_idle",
            "Number of idle database connections",
        )?;
        registry.register(Box::new(db_connections_idle.clone()))?;

        let db_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "db_query_duration_seconds",
                "Database query duration in seconds",
            )
            .namespace("acpms")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
            &["query_type"],
        )?;
        registry.register(Box::new(db_query_duration_seconds.clone()))?;

        // Worker metrics
        let worker_queue_depth = IntGauge::new(
            "acpms_worker_queue_depth",
            "Number of jobs waiting in the worker queue",
        )?;
        registry.register(Box::new(worker_queue_depth.clone()))?;

        let worker_active_jobs = IntGauge::new(
            "acpms_worker_active_jobs",
            "Number of jobs currently being processed by workers",
        )?;
        registry.register(Box::new(worker_active_jobs.clone()))?;

        let worker_jobs_total = IntCounterVec::new(
            Opts::new("worker_jobs_total", "Total number of worker jobs").namespace("acpms"),
            &["status"], // completed, failed, cancelled
        )?;
        registry.register(Box::new(worker_jobs_total.clone()))?;

        // Business metrics
        let tasks_created_total = IntCounterVec::new(
            Opts::new("tasks_created_total", "Total number of tasks created").namespace("acpms"),
            &["project_id"],
        )?;
        registry.register(Box::new(tasks_created_total.clone()))?;

        let task_attempts_total = IntCounterVec::new(
            Opts::new("task_attempts_total", "Total number of task attempts").namespace("acpms"),
            &["status"], // pending, running, success, failed
        )?;
        registry.register(Box::new(task_attempts_total.clone()))?;

        let projects_total =
            IntGauge::new("acpms_projects_total", "Total number of active projects")?;
        registry.register(Box::new(projects_total.clone()))?;

        // Deployment metrics
        let deployment_runs_total = IntCounterVec::new(
            Opts::new(
                "deployment_runs_total",
                "Total number of deployment runs by terminal/non-terminal status",
            )
            .namespace("acpms"),
            &["status", "environment"],
        )?;
        registry.register(Box::new(deployment_runs_total.clone()))?;

        let deployment_run_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "deployment_run_duration_seconds",
                "Deployment run duration in seconds",
            )
            .namespace("acpms")
            .buckets(vec![
                1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1200.0,
            ]),
            &["environment"],
        )?;
        registry.register(Box::new(deployment_run_duration_seconds.clone()))?;

        let deployment_failures_total = IntCounterVec::new(
            Opts::new(
                "deployment_failures_total",
                "Total number of failed deployment transitions by step",
            )
            .namespace("acpms"),
            &["step", "environment"],
        )?;
        registry.register(Box::new(deployment_failures_total.clone()))?;

        let rollback_runs_total = IntCounterVec::new(
            Opts::new(
                "rollback_runs_total",
                "Total number of rollback runs by result",
            )
            .namespace("acpms"),
            &["result", "environment"],
        )?;
        registry.register(Box::new(rollback_runs_total.clone()))?;

        // Agent auth metrics
        let auth_session_started_total = IntCounterVec::new(
            Opts::new(
                "auth_session_started_total",
                "Total number of initiated auth sessions by provider",
            )
            .namespace("acpms"),
            &["provider"],
        )?;
        registry.register(Box::new(auth_session_started_total.clone()))?;

        let auth_session_success_total = IntCounterVec::new(
            Opts::new(
                "auth_session_success_total",
                "Total number of successful auth sessions by provider",
            )
            .namespace("acpms"),
            &["provider"],
        )?;
        registry.register(Box::new(auth_session_success_total.clone()))?;

        let auth_session_failed_total = IntCounterVec::new(
            Opts::new(
                "auth_session_failed_total",
                "Total number of failed auth sessions by provider",
            )
            .namespace("acpms"),
            &["provider"],
        )?;
        registry.register(Box::new(auth_session_failed_total.clone()))?;

        let auth_session_timeout_total = IntCounterVec::new(
            Opts::new(
                "auth_session_timeout_total",
                "Total number of timed-out auth sessions by provider",
            )
            .namespace("acpms"),
            &["provider"],
        )?;
        registry.register(Box::new(auth_session_timeout_total.clone()))?;

        let repository_access_evaluations_total = IntCounterVec::new(
            Opts::new(
                "repository_access_evaluations_total",
                "Total number of repository access evaluations by source and resulting mode",
            )
            .namespace("acpms"),
            &["source", "provider", "access_mode", "verification_status"],
        )?;
        registry.register(Box::new(repository_access_evaluations_total.clone()))?;

        let repository_fork_operations_total = IntCounterVec::new(
            Opts::new(
                "repository_fork_operations_total",
                "Total number of repository fork-related operations by source, provider and result",
            )
            .namespace("acpms"),
            &["source", "provider", "result"],
        )?;
        registry.register(Box::new(repository_fork_operations_total.clone()))?;

        let repository_backfill_total = IntCounterVec::new(
            Opts::new(
                "repository_backfill_total",
                "Total number of legacy repository context backfill events by source and result",
            )
            .namespace("acpms"),
            &["source", "result"],
        )?;
        registry.register(Box::new(repository_backfill_total.clone()))?;

        let repository_attempt_blocks_total = IntCounterVec::new(
            Opts::new(
                "repository_attempt_blocks_total",
                "Total number of coding attempts blocked by repository access mode",
            )
            .namespace("acpms"),
            &["access_mode", "verification_status"],
        )?;
        registry.register(Box::new(repository_attempt_blocks_total.clone()))?;

        let openclaw_gateway_auth_total = IntCounterVec::new(
            Opts::new(
                "openclaw_gateway_auth_total",
                "Total number of OpenClaw gateway auth attempts by result",
            )
            .namespace("acpms"),
            &["result"],
        )?;
        registry.register(Box::new(openclaw_gateway_auth_total.clone()))?;

        let openclaw_event_stream_connections_total = IntCounterVec::new(
            Opts::new(
                "openclaw_event_stream_connections_total",
                "Total number of OpenClaw event stream connections by mode",
            )
            .namespace("acpms"),
            &["mode"],
        )?;
        registry.register(Box::new(openclaw_event_stream_connections_total.clone()))?;

        let openclaw_event_stream_active_connections = IntGauge::new(
            "acpms_openclaw_event_stream_active_connections",
            "Number of active OpenClaw event stream connections",
        )?;
        registry.register(Box::new(openclaw_event_stream_active_connections.clone()))?;

        let openclaw_event_stream_replay_events_total = IntCounterVec::new(
            Opts::new(
                "openclaw_event_stream_replay_events_total",
                "Total number of replayed OpenClaw events by stream mode",
            )
            .namespace("acpms"),
            &["mode"],
        )?;
        registry.register(Box::new(openclaw_event_stream_replay_events_total.clone()))?;

        let openclaw_event_stream_cursor_expired_total = IntCounterVec::new(
            Opts::new(
                "openclaw_event_stream_cursor_expired_total",
                "Total number of stale OpenClaw event stream cursor failures",
            )
            .namespace("acpms"),
            &["reason"],
        )?;
        registry.register(Box::new(openclaw_event_stream_cursor_expired_total.clone()))?;

        let openclaw_events_recorded_total = IntCounterVec::new(
            Opts::new(
                "openclaw_events_recorded_total",
                "Total number of OpenClaw gateway events persisted by event type",
            )
            .namespace("acpms"),
            &["event_type"],
        )?;
        registry.register(Box::new(openclaw_events_recorded_total.clone()))?;

        let openclaw_webhook_deliveries_total = IntCounterVec::new(
            Opts::new(
                "openclaw_webhook_deliveries_total",
                "Total number of OpenClaw webhook delivery attempts by result and status class",
            )
            .namespace("acpms"),
            &["result", "status_class"],
        )?;
        registry.register(Box::new(openclaw_webhook_deliveries_total.clone()))?;

        let openclaw_webhook_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "openclaw_webhook_duration_seconds",
                "OpenClaw webhook delivery duration in seconds",
            )
            .namespace("acpms")
            .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]),
            &["result"],
        )?;
        registry.register(Box::new(openclaw_webhook_duration_seconds.clone()))?;

        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            db_connections_active,
            db_connections_idle,
            db_query_duration_seconds,
            worker_queue_depth,
            worker_active_jobs,
            worker_jobs_total,
            tasks_created_total,
            task_attempts_total,
            projects_total,
            deployment_runs_total,
            deployment_run_duration_seconds,
            deployment_failures_total,
            rollback_runs_total,
            auth_session_started_total,
            auth_session_success_total,
            auth_session_failed_total,
            auth_session_timeout_total,
            repository_access_evaluations_total,
            repository_fork_operations_total,
            repository_backfill_total,
            repository_attempt_blocks_total,
            openclaw_gateway_auth_total,
            openclaw_event_stream_connections_total,
            openclaw_event_stream_active_connections,
            openclaw_event_stream_replay_events_total,
            openclaw_event_stream_cursor_expired_total,
            openclaw_events_recorded_total,
            openclaw_webhook_deliveries_total,
            openclaw_webhook_duration_seconds,
        })
    }

    /// Encode metrics in Prometheus text format
    #[allow(dead_code)]
    pub fn encode(&self) -> anyhow::Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

impl OpenClawGatewayMetricsObserver for Metrics {
    fn on_event_recorded(&self, event_type: &str) {
        self.openclaw_events_recorded_total
            .with_label_values(&[event_type])
            .inc();
    }

    fn on_webhook_delivery(&self, success: bool, status_code: Option<u16>, duration_seconds: f64) {
        let result = if success { "success" } else { "failure" };
        let status_class = match status_code {
            Some(code) => match code / 100 {
                2 => "2xx",
                3 => "3xx",
                4 => "4xx",
                5 => "5xx",
                _ => "other",
            },
            None => "none",
        };

        self.openclaw_webhook_deliveries_total
            .with_label_values(&[result, status_class])
            .inc();
        self.openclaw_webhook_duration_seconds
            .with_label_values(&[result])
            .observe(duration_seconds);
    }
}

#[cfg(test)]
mod tests {
    use acpms_services::OpenClawGatewayMetricsObserver;

    use super::Metrics;

    #[test]
    fn deployment_metrics_are_registered_and_encoded() {
        let metrics = Metrics::new().expect("metrics should initialize");

        metrics
            .deployment_runs_total
            .with_label_values(&["queued", "dev"])
            .inc();
        metrics
            .deployment_runs_total
            .with_label_values(&["success", "dev"])
            .inc();
        metrics
            .deployment_failures_total
            .with_label_values(&["healthcheck", "dev"])
            .inc();
        metrics
            .deployment_run_duration_seconds
            .with_label_values(&["dev"])
            .observe(12.5);
        metrics
            .rollback_runs_total
            .with_label_values(&["failed", "dev"])
            .inc();
        metrics
            .auth_session_started_total
            .with_label_values(&["openai-codex"])
            .inc();
        metrics
            .auth_session_success_total
            .with_label_values(&["openai-codex"])
            .inc();
        metrics
            .auth_session_failed_total
            .with_label_values(&["claude-code"])
            .inc();
        metrics
            .auth_session_timeout_total
            .with_label_values(&["gemini-cli"])
            .inc();
        metrics
            .repository_access_evaluations_total
            .with_label_values(&["import_preflight", "github", "analysis_only", "verified"])
            .inc();
        metrics
            .repository_fork_operations_total
            .with_label_values(&["project_create_fork", "github", "success"])
            .inc();
        metrics
            .repository_backfill_total
            .with_label_values(&["project_get", "queued"])
            .inc();
        metrics
            .repository_attempt_blocks_total
            .with_label_values(&["unknown", "unknown"])
            .inc();
        metrics
            .openclaw_gateway_auth_total
            .with_label_values(&["success"])
            .inc();
        metrics
            .openclaw_event_stream_connections_total
            .with_label_values(&["resume"])
            .inc();
        metrics.openclaw_event_stream_active_connections.inc();
        metrics
            .openclaw_event_stream_replay_events_total
            .with_label_values(&["resume"])
            .inc_by(3);
        metrics
            .openclaw_event_stream_cursor_expired_total
            .with_label_values(&["stale_cursor"])
            .inc();
        metrics.on_event_recorded("attempt.completed");
        metrics.on_webhook_delivery(true, Some(200), 0.12);

        let encoded = metrics.encode().expect("metrics should encode");

        assert!(encoded.contains("acpms_deployment_runs_total"));
        assert!(encoded.contains("acpms_deployment_run_duration_seconds_bucket"));
        assert!(encoded.contains("acpms_deployment_failures_total"));
        assert!(encoded.contains("acpms_rollback_runs_total"));
        assert!(encoded.contains("acpms_auth_session_started_total"));
        assert!(encoded.contains("acpms_auth_session_success_total"));
        assert!(encoded.contains("acpms_auth_session_failed_total"));
        assert!(encoded.contains("acpms_auth_session_timeout_total"));
        assert!(encoded.contains("acpms_repository_access_evaluations_total"));
        assert!(encoded.contains("acpms_repository_fork_operations_total"));
        assert!(encoded.contains("acpms_repository_backfill_total"));
        assert!(encoded.contains("acpms_repository_attempt_blocks_total"));
        assert!(encoded.contains("acpms_openclaw_gateway_auth_total"));
        assert!(encoded.contains("acpms_openclaw_event_stream_connections_total"));
        assert!(encoded.contains("acpms_openclaw_event_stream_active_connections"));
        assert!(encoded.contains("acpms_openclaw_event_stream_replay_events_total"));
        assert!(encoded.contains("acpms_openclaw_event_stream_cursor_expired_total"));
        assert!(encoded.contains("acpms_openclaw_events_recorded_total"));
        assert!(encoded.contains("acpms_openclaw_webhook_deliveries_total"));
        assert!(encoded.contains("acpms_openclaw_webhook_duration_seconds_bucket"));
        assert!(encoded.contains("environment=\"dev\""));
    }
}
