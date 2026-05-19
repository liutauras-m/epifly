//! Prometheus metrics for RustFS operations.
//!
//! Metrics registered here:
//!  rustfs_op_latency_seconds{op, tenant}              — histogram
//!  rustfs_op_errors_total{op, code, tenant}           — counter
//!  rustfs_bytes_in_total{tenant}                      — counter
//!  rustfs_bytes_out_total{tenant}                     — counter
//!  rustfs_storage_used_bytes{tenant}                  — gauge (updated by quota service)
//!  tenant_storage_fallback_total{result}              — counter (dev fallback occurrences)
//!  storage_ops_total{op, result, tier}                — counter (storage operation outcomes)
//!  tenant_onboarding_total{kind}                      — counter (successful tenant provisions)
//!  tenant_onboarding_marker_failed_total              — counter (_meta/seeded write failures)

use prometheus::{
    CounterVec, GaugeVec, HistogramVec, IntCounter, IntCounterVec, Opts, Registry,
    histogram_opts,
};
use std::sync::Arc;

pub struct RustFsMetrics {
    pub op_latency: HistogramVec,
    pub op_errors: CounterVec,
    pub bytes_in: CounterVec,
    pub bytes_out: CounterVec,
    pub storage_used: GaugeVec,
    /// Incremented whenever the dev-fallback-to-root-creds path is taken.
    /// Any non-zero value in prod should page on-call.
    pub storage_fallback_total: IntCounterVec,
    /// Per-operation outcome counter. Labels: op, result, tier (plan tier — not tenant id).
    pub storage_ops_total: IntCounterVec,
    /// Successful tenant provisions, labeled by kind (normal|system).
    pub tenant_onboarding_total: IntCounterVec,
    /// _meta/seeded marker write failures (non-fatal — DB record is authoritative).
    pub tenant_onboarding_marker_failed_total: IntCounter,
}

impl RustFsMetrics {
    pub fn register(registry: &Registry) -> Arc<Self> {
        let op_latency = HistogramVec::new(
            histogram_opts!(
                "rustfs_op_latency_seconds",
                "RustFS operation latency in seconds",
                vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
            ),
            &["op", "tenant"],
        )
        .unwrap();

        let op_errors = CounterVec::new(
            Opts::new("rustfs_op_errors_total", "RustFS operation errors"),
            &["op", "code", "tenant"],
        )
        .unwrap();

        let bytes_in = CounterVec::new(
            Opts::new("rustfs_bytes_in_total", "Bytes written to RustFS per tenant"),
            &["tenant"],
        )
        .unwrap();

        let bytes_out = CounterVec::new(
            Opts::new("rustfs_bytes_out_total", "Bytes read from RustFS per tenant"),
            &["tenant"],
        )
        .unwrap();

        let storage_used = GaugeVec::new(
            Opts::new("rustfs_storage_used_bytes", "Storage used per tenant in bytes"),
            &["tenant"],
        )
        .unwrap();

        let storage_fallback_total = IntCounterVec::new(
            Opts::new(
                "tenant_storage_fallback_total",
                "Number of times the dev root-cred fallback was used (should be 0 in prod)",
            ),
            &["result"],
        )
        .unwrap();

        let storage_ops_total = IntCounterVec::new(
            Opts::new("storage_ops_total", "Storage operation outcomes by op, result, and plan tier"),
            &["op", "result", "tier"],
        )
        .unwrap();

        let tenant_onboarding_total = IntCounterVec::new(
            Opts::new("tenant_onboarding_total", "Successful tenant provisioning operations"),
            &["kind"],
        )
        .unwrap();

        let tenant_onboarding_marker_failed_total = IntCounter::new(
            "tenant_onboarding_marker_failed_total",
            "Number of times the _meta/seeded marker write failed (non-fatal)",
        )
        .unwrap();

        let _ = registry.register(Box::new(op_latency.clone()));
        let _ = registry.register(Box::new(op_errors.clone()));
        let _ = registry.register(Box::new(bytes_in.clone()));
        let _ = registry.register(Box::new(bytes_out.clone()));
        let _ = registry.register(Box::new(storage_used.clone()));
        let _ = registry.register(Box::new(storage_fallback_total.clone()));
        let _ = registry.register(Box::new(storage_ops_total.clone()));
        let _ = registry.register(Box::new(tenant_onboarding_total.clone()));
        let _ = registry.register(Box::new(tenant_onboarding_marker_failed_total.clone()));

        Arc::new(Self {
            op_latency,
            op_errors,
            bytes_in,
            bytes_out,
            storage_used,
            storage_fallback_total,
            storage_ops_total,
            tenant_onboarding_total,
            tenant_onboarding_marker_failed_total,
        })
    }

    pub fn record_op(&self, op: &str, tenant: &str, duration_secs: f64) {
        self.op_latency
            .with_label_values(&[op, tenant])
            .observe(duration_secs);
    }

    pub fn record_error(&self, op: &str, code: &str, tenant: &str) {
        self.op_errors
            .with_label_values(&[op, code, tenant])
            .inc();
    }

    pub fn record_bytes_in(&self, tenant: &str, bytes: f64) {
        self.bytes_in.with_label_values(&[tenant]).inc_by(bytes);
    }

    pub fn record_bytes_out(&self, tenant: &str, bytes: f64) {
        self.bytes_out.with_label_values(&[tenant]).inc_by(bytes);
    }

    pub fn set_storage_used(&self, tenant: &str, bytes: f64) {
        self.storage_used.with_label_values(&[tenant]).set(bytes);
    }

    pub fn record_storage_fallback(&self, result: &str) {
        self.storage_fallback_total.with_label_values(&[result]).inc();
    }

    /// Record a storage operation outcome. `tier` is the tenant's plan tier (e.g. "free", "pro").
    pub fn record_storage_op(&self, op: &str, ok: bool, tier: &str) {
        let result = if ok { "ok" } else { "err" };
        self.storage_ops_total.with_label_values(&[op, result, tier]).inc();
    }

    pub fn record_onboarding(&self, kind: &str) {
        self.tenant_onboarding_total.with_label_values(&[kind]).inc();
    }

    pub fn record_onboarding_marker_failed(&self) {
        self.tenant_onboarding_marker_failed_total.inc();
    }
}
