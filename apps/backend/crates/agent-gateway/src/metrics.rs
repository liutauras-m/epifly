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
//!  routing_latency_ms{stage}                          — histogram (router stage latency)
//!  tools_per_turn                                     — histogram (tool definitions served per chat turn)
//!  forced_capability_hit_rate{result}                 — counter (pinned_kept|dropped|none)
//!  embedding_cache_hit_rate{result}                   — counter (hit|miss; wired by PR 2.B.3.1)
//!  low_confidence_turns_total                         — counter (router max_score < threshold; PR 2.A.3.1)

use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec, Opts, Registry,
};
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
    /// Active embedding model dimensionality, labeled by model name.
    pub embedding_dims: IntGaugeVec,
}

/// Routing-decision metrics observed by `stream_agent` / `blocking_agent`.
///
/// Per Phase 1.6.3 of `docs/plan.md`. Counters that depend on still-unimplemented
/// PR 2 features (`forced_capability`, embedding cache, confidence threshold) are
/// registered now so the `/metrics` contract is stable; they will be incremented
/// once the corresponding code paths land.
pub struct RouterMetrics {
    /// Latency of each routing stage in milliseconds.
    /// `stage` labels: `semantic` | `lexical` | `forced_pin` | `merge` | `total`.
    pub routing_latency_ms: HistogramVec,
    /// Number of tool definitions served per turn (after truncation).
    pub tools_per_turn: Histogram,
    /// Outcome of any `forced_capability` request.
    /// `result` labels: `pinned_kept` | `dropped` | `none` (no forced cap requested).
    pub forced_capability_hit_rate: IntCounterVec,
    /// Tool-embedding cache outcome at registry load / lookup.
    /// `result` labels: `hit` | `miss`. Wired by PR 2.B.3.1.
    /// Read via `record_embedding_cache()`; field is registered with Prometheus
    /// at boot so the `/metrics` contract is stable before PR 2 lands.
    #[allow(dead_code)]
    pub embedding_cache_hit_rate: IntCounterVec,
    /// Turns where the router's `max_score` fell below the configured threshold
    /// (PR 2.A.3.1). Registered now for contract stability; `/metrics` will show
    /// `low_confidence_turns_total 0` until PR 2.A.3.1 wires the increment.
    #[allow(dead_code)]
    pub low_confidence_turns_total: IntCounter,
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
        let _ = registry.register(Box::new(embedding_dims.clone()));

        Arc::new(Self {
            storage_fallback_total,
            tenant_onboarding_total,
            tenant_onboarding_marker_failed_total,
            zitadel_cache_hits_total,
            zitadel_cache_misses_total,
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

    pub fn set_embedding_dims(&self, model: &str, dims: i64) {
        self.embedding_dims.with_label_values(&[model]).set(dims);
    }
}

impl RouterMetrics {
    pub fn register(registry: &Registry) -> Arc<Self> {
        // Latency buckets tuned for embedding lookups + small in-process work.
        // Most stages take 1–100 ms; semantic with a cold cache can spike to ~1 s.
        let latency_buckets = vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];

        let routing_latency_ms = HistogramVec::new(
            HistogramOpts::new(
                "routing_latency_ms",
                "Per-stage latency of the semantic capability router (milliseconds)",
            )
            .buckets(latency_buckets),
            &["stage"],
        )
        .unwrap();

        let tools_per_turn = Histogram::with_opts(
            HistogramOpts::new(
                "tools_per_turn",
                "Number of tool definitions served to the LLM per turn (after truncation)",
            )
            .buckets(vec![0.0, 1.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0]),
        )
        .unwrap();

        let forced_capability_hit_rate = IntCounterVec::new(
            Opts::new(
                "forced_capability_hit_rate",
                "Outcome of forced_capability pinning per turn (pinned_kept|dropped|none)",
            ),
            &["result"],
        )
        .unwrap();

        let embedding_cache_hit_rate = IntCounterVec::new(
            Opts::new(
                "embedding_cache_hit_rate",
                "Tool-embedding cache outcomes (hit|miss); wired by PR 2.B.3.1",
            ),
            &["result"],
        )
        .unwrap();

        let low_confidence_turns_total = IntCounter::new(
            "low_confidence_turns_total",
            "Turns where router max_score fell below the configured threshold (PR 2.A.3.1)",
        )
        .unwrap();

        let _ = registry.register(Box::new(routing_latency_ms.clone()));
        let _ = registry.register(Box::new(tools_per_turn.clone()));
        let _ = registry.register(Box::new(forced_capability_hit_rate.clone()));
        let _ = registry.register(Box::new(embedding_cache_hit_rate.clone()));
        let _ = registry.register(Box::new(low_confidence_turns_total.clone()));

        Arc::new(Self {
            routing_latency_ms,
            tools_per_turn,
            forced_capability_hit_rate,
            embedding_cache_hit_rate,
            low_confidence_turns_total,
        })
    }

    /// Observe latency in milliseconds for a named routing stage.
    pub fn observe_stage_ms(&self, stage: &str, ms: f64) {
        self.routing_latency_ms
            .with_label_values(&[stage])
            .observe(ms);
    }

    /// Observe the count of tools served for one turn.
    pub fn observe_tools_per_turn(&self, count: usize) {
        self.tools_per_turn.observe(count as f64);
    }

    /// Record the outcome of a forced_capability request.
    /// `result` is one of: `pinned_kept`, `dropped`, `none`.
    pub fn record_forced_capability(&self, result: &str) {
        self.forced_capability_hit_rate
            .with_label_values(&[result])
            .inc();
    }

    /// Record a tool-embedding cache outcome (`hit` | `miss`). Wired by PR 2.B.3.1.
    #[allow(dead_code)]
    pub fn record_embedding_cache(&self, result: &str) {
        self.embedding_cache_hit_rate
            .with_label_values(&[result])
            .inc();
    }

    /// Record a turn where router `max_score` fell below the confidence threshold.
    /// Wired by PR 2.A.3.1.
    #[allow(dead_code)]
    pub fn record_low_confidence_turn(&self) {
        self.low_confidence_turns_total.inc();
    }
}
