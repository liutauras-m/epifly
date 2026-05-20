//! Prometheus metrics for the agent gateway.
//!
//! Metrics registered here:
//!  tenant_storage_fallback_total{result}              — counter (dev fallback occurrences)
//!  tenant_onboarding_total{kind}                      — counter (successful tenant provisions)
//!  tenant_onboarding_marker_failed_total              — counter (_meta/seeded write failures)
//!  zitadel_introspection_cache_hits_total             — counter (token cache hits)
//!  zitadel_introspection_cache_misses_total           — counter (token cache misses)
//!  plan_clamp_total{tier,parameter}                   — counter (requests where plan cap is applied)
//!  embedding_dims{model}                              — gauge (active embedding model dimensionality)

use prometheus::{IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry};
use std::sync::Arc;

pub struct RustFsMetrics {
    /// Incremented whenever the dev-fallback-to-root-creds path is taken.
    pub storage_fallback_total: IntCounterVec,
    /// Successful tenant provisions, labeled by kind (normal|system).
    pub tenant_onboarding_total: IntCounterVec,
    /// _meta/seeded marker write failures (non-fatal — DB record is authoritative).
    pub tenant_onboarding_marker_failed_total: IntCounter,
    /// Zitadel token introspection cache hits.
    pub zitadel_cache_hits_total: IntCounter,
    /// Zitadel token introspection cache misses.
    pub zitadel_cache_misses_total: IntCounter,
    /// Requests where a plan-tier cap was applied, labeled by tier and parameter name.
    pub plan_clamp_total: IntCounterVec,
    /// Active embedding model dimensionality, labeled by model name.
    pub embedding_dims: IntGaugeVec,
}

impl RustFsMetrics {
    pub fn register(registry: &Registry) -> Arc<Self> {
        let storage_fallback_total = IntCounterVec::new(
            Opts::new(
                "tenant_storage_fallback_total",
                "Number of times the dev root-cred fallback was used (should be 0 in prod)",
            ),
            &["result"],
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

        let zitadel_cache_hits_total = IntCounter::new(
            "zitadel_introspection_cache_hits_total",
            "Zitadel token introspection results served from cache",
        )
        .unwrap();

        let zitadel_cache_misses_total = IntCounter::new(
            "zitadel_introspection_cache_misses_total",
            "Zitadel token introspection results fetched from remote",
        )
        .unwrap();

        let plan_clamp_total = IntCounterVec::new(
            Opts::new(
                "plan_clamp_total",
                "Requests where a plan-tier hard cap was applied to a parameter",
            ),
            &["tier", "parameter"],
        )
        .unwrap();

        let embedding_dims = IntGaugeVec::new(
            Opts::new(
                "embedding_dims",
                "Active embedding model output dimensionality",
            ),
            &["model"],
        )
        .unwrap();

        let _ = registry.register(Box::new(storage_fallback_total.clone()));
        let _ = registry.register(Box::new(tenant_onboarding_total.clone()));
        let _ = registry.register(Box::new(tenant_onboarding_marker_failed_total.clone()));
        let _ = registry.register(Box::new(zitadel_cache_hits_total.clone()));
        let _ = registry.register(Box::new(zitadel_cache_misses_total.clone()));
        let _ = registry.register(Box::new(plan_clamp_total.clone()));
        let _ = registry.register(Box::new(embedding_dims.clone()));

        Arc::new(Self {
            storage_fallback_total,
            tenant_onboarding_total,
            tenant_onboarding_marker_failed_total,
            zitadel_cache_hits_total,
            zitadel_cache_misses_total,
            plan_clamp_total,
            embedding_dims,
        })
    }

    pub fn record_storage_fallback(&self, result: &str) {
        self.storage_fallback_total.with_label_values(&[result]).inc();
    }

    pub fn record_onboarding(&self, kind: &str) {
        self.tenant_onboarding_total.with_label_values(&[kind]).inc();
    }

    pub fn record_onboarding_marker_failed(&self) {
        self.tenant_onboarding_marker_failed_total.inc();
    }

    pub fn record_zitadel_cache_hit(&self) {
        self.zitadel_cache_hits_total.inc();
    }

    pub fn record_zitadel_cache_miss(&self) {
        self.zitadel_cache_misses_total.inc();
    }

    pub fn record_plan_clamp(&self, tier: &str, parameter: &str) {
        self.plan_clamp_total.with_label_values(&[tier, parameter]).inc();
    }

    pub fn set_embedding_dims(&self, model: &str, dims: i64) {
        self.embedding_dims.with_label_values(&[model]).set(dims);
    }
}
