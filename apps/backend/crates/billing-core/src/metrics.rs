/// Prometheus counters and histograms for billing and quota operations.
use prometheus::{
    CounterVec, HistogramVec, Registry, register_counter_vec_with_registry,
    register_histogram_vec_with_registry,
};
use std::sync::OnceLock;

static QUOTA_DENIED: OnceLock<CounterVec> = OnceLock::new();
static WEBHOOK_EVENTS: OnceLock<CounterVec> = OnceLock::new();
static OIDC_VERIFY_DURATION: OnceLock<HistogramVec> = OnceLock::new();

pub fn register(registry: &Registry) {
    let quota_denied = register_counter_vec_with_registry!(
        "conusai_quota_denied_total",
        "Number of requests denied due to quota exhaustion",
        &["action", "plan"],
        registry
    )
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to register conusai_quota_denied_total");
        CounterVec::new(
            prometheus::Opts::new("conusai_quota_denied_total_fallback", "fallback"),
            &["action", "plan"],
        )
        .unwrap()
    });
    let _ = QUOTA_DENIED.set(quota_denied);

    let webhook_events = register_counter_vec_with_registry!(
        "conusai_billing_webhook_total",
        "Total billing webhook events received",
        &["event", "result"],
        registry
    )
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to register conusai_billing_webhook_total");
        CounterVec::new(
            prometheus::Opts::new("conusai_billing_webhook_total_fallback", "fallback"),
            &["event", "result"],
        )
        .unwrap()
    });
    let _ = WEBHOOK_EVENTS.set(webhook_events);

    let oidc_duration = register_histogram_vec_with_registry!(
        "conusai_oidc_verify_duration_seconds",
        "Duration of OIDC token verification requests",
        &["provider"],
        registry
    )
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to register conusai_oidc_verify_duration_seconds");
        HistogramVec::new(
            prometheus::HistogramOpts::new("conusai_oidc_verify_duration_fallback", "fallback"),
            &["provider"],
        )
        .unwrap()
    });
    let _ = OIDC_VERIFY_DURATION.set(oidc_duration);
}

pub fn inc_quota_denied(action: &str, plan: &str) {
    if let Some(c) = QUOTA_DENIED.get() {
        c.with_label_values(&[action, plan]).inc();
    }
}

pub fn inc_webhook_event(event: &str, result: &str) {
    if let Some(c) = WEBHOOK_EVENTS.get() {
        c.with_label_values(&[event, result]).inc();
    }
}

pub fn observe_oidc_duration(provider: &str, secs: f64) {
    if let Some(h) = OIDC_VERIFY_DURATION.get() {
        h.with_label_values(&[provider]).observe(secs);
    }
}
