use prometheus::{
    Counter, Histogram, HistogramOpts, IntGauge, Opts, Registry,
    register_counter_with_registry, register_histogram_with_registry,
    register_int_gauge_with_registry,
};
use std::sync::Arc;

/// Integration monitoring metrics for GitLab, webhooks, and OAuth
#[derive(Clone)]
pub struct IntegrationMetrics {
    // GitLab API metrics
    pub gitlab_api_requests_total: Counter,
    pub gitlab_api_errors_total: Counter,
    pub gitlab_api_duration_seconds: Histogram,
    pub gitlab_api_rate_limit_remaining: IntGauge,

    // Webhook metrics
    pub webhook_events_received_total: Counter,
    pub webhook_events_processed_total: Counter,
    pub webhook_events_failed_total: Counter,
    pub webhook_processing_duration_seconds: Histogram,
    pub webhook_queue_size: IntGauge,
    pub webhook_retry_attempts_total: Counter,

    // OAuth metrics
    pub oauth_authorizations_total: Counter,
    pub oauth_token_exchanges_total: Counter,
    pub oauth_errors_total: Counter,
    pub oauth_tokens_stored: IntGauge,

    // Sync metrics
    pub gitlab_sync_operations_total: Counter,
    pub gitlab_sync_duration_seconds: Histogram,
    pub gitlab_sync_errors_total: Counter,
}

impl IntegrationMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            // GitLab API metrics
            gitlab_api_requests_total: register_counter_with_registry!(
                Opts::new("gitlab_api_requests_total", "Total GitLab API requests"),
                registry
            )?,
            gitlab_api_errors_total: register_counter_with_registry!(
                Opts::new("gitlab_api_errors_total", "Total GitLab API errors"),
                registry
            )?,
            gitlab_api_duration_seconds: register_histogram_with_registry!(
                HistogramOpts::new("gitlab_api_duration_seconds", "GitLab API request duration")
                    .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
                registry
            )?,
            gitlab_api_rate_limit_remaining: register_int_gauge_with_registry!(
                Opts::new("gitlab_api_rate_limit_remaining", "GitLab API rate limit remaining"),
                registry
            )?,

            // Webhook metrics
            webhook_events_received_total: register_counter_with_registry!(
                Opts::new("webhook_events_received_total", "Total webhook events received"),
                registry
            )?,
            webhook_events_processed_total: register_counter_with_registry!(
                Opts::new("webhook_events_processed_total", "Total webhook events processed successfully"),
                registry
            )?,
            webhook_events_failed_total: register_counter_with_registry!(
                Opts::new("webhook_events_failed_total", "Total webhook events failed"),
                registry
            )?,
            webhook_processing_duration_seconds: register_histogram_with_registry!(
                HistogramOpts::new("webhook_processing_duration_seconds", "Webhook processing duration")
                    .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]),
                registry
            )?,
            webhook_queue_size: register_int_gauge_with_registry!(
                Opts::new("webhook_queue_size", "Current webhook queue size"),
                registry
            )?,
            webhook_retry_attempts_total: register_counter_with_registry!(
                Opts::new("webhook_retry_attempts_total", "Total webhook retry attempts"),
                registry
            )?,

            // OAuth metrics
            oauth_authorizations_total: register_counter_with_registry!(
                Opts::new("oauth_authorizations_total", "Total OAuth authorization requests"),
                registry
            )?,
            oauth_token_exchanges_total: register_counter_with_registry!(
                Opts::new("oauth_token_exchanges_total", "Total OAuth token exchanges"),
                registry
            )?,
            oauth_errors_total: register_counter_with_registry!(
                Opts::new("oauth_errors_total", "Total OAuth errors"),
                registry
            )?,
            oauth_tokens_stored: register_int_gauge_with_registry!(
                Opts::new("oauth_tokens_stored", "Number of OAuth tokens stored"),
                registry
            )?,

            // Sync metrics
            gitlab_sync_operations_total: register_counter_with_registry!(
                Opts::new("gitlab_sync_operations_total", "Total GitLab sync operations"),
                registry
            )?,
            gitlab_sync_duration_seconds: register_histogram_with_registry!(
                HistogramOpts::new("gitlab_sync_duration_seconds", "GitLab sync duration")
                    .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]),
                registry
            )?,
            gitlab_sync_errors_total: register_counter_with_registry!(
                Opts::new("gitlab_sync_errors_total", "Total GitLab sync errors"),
                registry
            )?,
        })
    }

    /// Record GitLab API request
    pub fn record_gitlab_api_request(&self, duration_seconds: f64, success: bool) {
        self.gitlab_api_requests_total.inc();
        self.gitlab_api_duration_seconds.observe(duration_seconds);
        if !success {
            self.gitlab_api_errors_total.inc();
        }
    }

    /// Record webhook event received
    pub fn record_webhook_received(&self, event_type: &str) {
        self.webhook_events_received_total.inc();
    }

    /// Record webhook processing result
    pub fn record_webhook_processed(&self, duration_seconds: f64, success: bool) {
        self.webhook_processing_duration_seconds.observe(duration_seconds);
        if success {
            self.webhook_events_processed_total.inc();
        } else {
            self.webhook_events_failed_total.inc();
        }
    }

    /// Record OAuth authorization
    pub fn record_oauth_authorization(&self) {
        self.oauth_authorizations_total.inc();
    }

    /// Record OAuth token exchange
    pub fn record_oauth_token_exchange(&self, success: bool) {
        self.oauth_token_exchanges_total.inc();
        if !success {
            self.oauth_errors_total.inc();
        }
    }

    /// Record GitLab sync operation
    pub fn record_gitlab_sync(&self, duration_seconds: f64, success: bool) {
        self.gitlab_sync_operations_total.inc();
        self.gitlab_sync_duration_seconds.observe(duration_seconds);
        if !success {
            self.gitlab_sync_errors_total.inc();
        }
    }
}
