//! Prometheus metrics for RustFS operations.
//!
//! Metrics registered here:
//!  rustfs_op_latency_seconds{op, tenant}   — histogram
//!  rustfs_op_errors_total{op, code, tenant} — counter
//!  rustfs_bytes_in_total{tenant}            — counter
//!  rustfs_bytes_out_total{tenant}           — counter
//!  rustfs_storage_used_bytes{tenant}        — gauge (updated by quota service)

use prometheus::{
    CounterVec, Gauge, GaugeVec, HistogramVec, Opts, Registry,
    histogram_opts,
};
use std::sync::Arc;

pub struct RustFsMetrics {
    pub op_latency: HistogramVec,
    pub op_errors: CounterVec,
    pub bytes_in: CounterVec,
    pub bytes_out: CounterVec,
    pub storage_used: GaugeVec,
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

        let _ = registry.register(Box::new(op_latency.clone()));
        let _ = registry.register(Box::new(op_errors.clone()));
        let _ = registry.register(Box::new(bytes_in.clone()));
        let _ = registry.register(Box::new(bytes_out.clone()));
        let _ = registry.register(Box::new(storage_used.clone()));

        Arc::new(Self {
            op_latency,
            op_errors,
            bytes_in,
            bytes_out,
            storage_used,
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
}
