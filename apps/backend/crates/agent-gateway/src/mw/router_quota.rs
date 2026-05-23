//! Tower middleware that enforces per-turn semantic routing quotas.
//!
//! Reads `TenantContext` from the request extensions and rejects the request
//! when the tool budget would be exceeded. The actual counting happens at
//! the router level; this middleware enforces the hard cap on the HTTP path.
//!
//! Also enforces daily quota limits via `QuotaChecker` for agent/chat routes,
//! returning 429 with upgrade URL when the plan's daily cap is exceeded.

use agent_core::context::tenant::PlanLimits;
use axum::{
    body::Body,
    http::{Request, Response, StatusCode, header},
};
use billing_core::{events::ActionType, quota::QuotaChecker};
use crate::mw::tenant::ResolvedTenant;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Default per-turn tool cap (number of tool definitions sent to the LLM).
pub const DEFAULT_MAX_TOOLS_PER_TURN: usize = 25;
/// Default per-turn tool invocation cap.
/// 25 allows bulk-style operations (e.g. deleting many workspace nodes) to
/// complete without hitting the limit. Override via `CONUSAI_MAX_INVOKES_PER_TURN`.
pub const DEFAULT_MAX_INVOKES_PER_TURN: usize = 25;
/// Default minimum cosine similarity score for confident routing (PR 2.A.3.1).
/// Turns where `max_score < MIN_CONFIDENCE` are tagged low-confidence; the router
/// still serves all lexical + forced-pin results but bumps the Prometheus counter.
pub const DEFAULT_MIN_CONFIDENCE: f64 = 0.60;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RouterQuotaConfig {
    pub max_tools_per_turn: usize,
    pub max_invokes_per_turn: usize,
    /// Minimum cosine similarity score [0.0, 1.0] for a turn to be considered
    /// high-confidence by the semantic router (PR 2.A.3.1).
    /// Turns below this threshold are flagged via `low_confidence_turns_total`.
    pub min_confidence: f64,
    /// Optional daily quota enforcer. When set, agent/chat routes check the
    /// per-day turn limit before passing through.
    pub quota: Option<Arc<QuotaChecker>>,
    /// URL shown in 429 bodies to direct users to the upgrade page.
    pub upgrade_url: String,
}

impl std::fmt::Debug for RouterQuotaConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterQuotaConfig")
            .field("max_tools_per_turn", &self.max_tools_per_turn)
            .field("max_invokes_per_turn", &self.max_invokes_per_turn)
            .field("min_confidence", &self.min_confidence)
            .field("quota_enabled", &self.quota.is_some())
            .field("upgrade_url", &self.upgrade_url)
            .finish()
    }
}

impl Default for RouterQuotaConfig {
    fn default() -> Self {
        Self {
            max_tools_per_turn: DEFAULT_MAX_TOOLS_PER_TURN,
            max_invokes_per_turn: DEFAULT_MAX_INVOKES_PER_TURN,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            quota: None,
            upgrade_url: "/account/billing".into(),
        }
    }
}

impl RouterQuotaConfig {
    pub fn from_env() -> Self {
        fn env_usize(key: &str, default: usize) -> usize {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        let upgrade_url = std::env::var("BILLING_RETURN_URL")
            .unwrap_or_else(|_| "/account/billing".into());
        let min_confidence = std::env::var("CONUSAI_ROUTER_MIN_CONFIDENCE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(DEFAULT_MIN_CONFIDENCE)
            .clamp(0.0, 1.0);
        Self {
            max_tools_per_turn: env_usize("CONUSAI_MAX_TOOLS_PER_TURN", DEFAULT_MAX_TOOLS_PER_TURN),
            max_invokes_per_turn: env_usize(
                "CONUSAI_MAX_INVOKES_PER_TURN",
                DEFAULT_MAX_INVOKES_PER_TURN,
            ),
            min_confidence,
            quota: None,
            upgrade_url,
        }
    }

    pub fn with_quota(mut self, quota: Arc<QuotaChecker>) -> Self {
        self.quota = Some(quota);
        self
    }

    /// Override the confidence threshold at which the router falls back to the
    /// lexical + top-3 ensemble (PR 2.A.3.1). Useful for tests and per-deploy
    /// tuning via `RouterQuotaLayer::with_config(…)`.
    #[allow(dead_code)]
    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
        self
    }
}

// ── Layer ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RouterQuotaLayer {
    cfg: Arc<RouterQuotaConfig>,
}

impl RouterQuotaLayer {
    pub fn new(cfg: RouterQuotaConfig) -> Self {
        Self { cfg: Arc::new(cfg) }
    }
}

impl<S> Layer<S> for RouterQuotaLayer {
    type Service = RouterQuotaMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RouterQuotaMiddleware {
            inner,
            cfg: Arc::clone(&self.cfg),
        }
    }
}

// ── Service ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RouterQuotaMiddleware<S> {
    inner: S,
    cfg: Arc<RouterQuotaConfig>,
}

impl<S> Service<Request<Body>> for RouterQuotaMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let cfg = Arc::clone(&self.cfg);
        let mut inner = self.inner.clone();
        Box::pin(async move {
            // Check daily quota for agent/chat completion routes.
            if let Some(ref quota) = cfg.quota {
                let path = req.uri().path();
                let is_agent_route = path.starts_with("/v1/agent/")
                    || path.starts_with("/v1/chat/");
                if is_agent_route {
                    if let Some(ResolvedTenant(ctx)) = req.extensions().get::<ResolvedTenant>() {
                        let decision = quota
                            .check(&ctx.tenant_id, &ctx.plan, &ActionType::AgentTurn, 1)
                            .await;
                        if !decision.allowed {
                            let plan_tier = format!("{}", ctx.plan);
                            let upgrade_url = cfg.upgrade_url.clone();
                            let reason = decision.reason.unwrap_or_default();
                            let body = serde_json::json!({
                                "code": "quota_exceeded",
                                "message": reason,
                                "plan_tier": plan_tier,
                                "upgrade_url": upgrade_url,
                            })
                            .to_string();
                            let mut resp = Response::new(Body::from(body));
                            *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
                            resp.headers_mut().insert(
                                header::CONTENT_TYPE,
                                "application/json".parse().unwrap(),
                            );
                            if let Some(reset_at) = decision.reset_at {
                                let secs = (reset_at - chrono::Utc::now()).num_seconds().max(0);
                                if let Ok(v) = secs.to_string().parse() {
                                    resp.headers_mut().insert(header::RETRY_AFTER, v);
                                }
                            }
                            return Ok(resp);
                        }
                    }
                }
            }

            // Override tool caps from per-plan limits when available (injected by enforce_plan).
            let effective_cfg = if let Some(plan_limits) = req.extensions().get::<PlanLimits>().copied() {
                Arc::new(RouterQuotaConfig {
                    max_tools_per_turn: plan_limits.max_tools_per_turn,
                    max_invokes_per_turn: plan_limits.max_invokes_per_turn,
                    min_confidence: cfg.min_confidence,
                    quota: cfg.quota.clone(),
                    upgrade_url: cfg.upgrade_url.clone(),
                })
            } else {
                Arc::clone(&cfg)
            };

            let (mut parts, body) = req.into_parts();
            parts.extensions.insert(effective_cfg);
            inner.call(Request::from_parts(parts, body)).await
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::StatusCode, routing::get};
    use tower::ServiceExt;

    // ── RouterQuotaConfig defaults ────────────────────────────────────────

    #[test]
    fn default_config_values() {
        let cfg = RouterQuotaConfig::default();
        assert_eq!(cfg.max_tools_per_turn, DEFAULT_MAX_TOOLS_PER_TURN);
        assert_eq!(cfg.max_invokes_per_turn, DEFAULT_MAX_INVOKES_PER_TURN);
    }

    #[test]
    fn from_env_returns_defaults_when_vars_absent() {
        // Ensure the env vars are absent for this test.
        // SAFETY: single-threaded test context.
        unsafe {
            std::env::remove_var("CONUSAI_MAX_TOOLS_PER_TURN");
            std::env::remove_var("CONUSAI_MAX_INVOKES_PER_TURN");
        }
        let cfg = RouterQuotaConfig::from_env();
        assert_eq!(cfg.max_tools_per_turn, DEFAULT_MAX_TOOLS_PER_TURN);
        assert_eq!(cfg.max_invokes_per_turn, DEFAULT_MAX_INVOKES_PER_TURN);
    }

    /// `from_env` falls back to the provided default when parsing fails.
    #[test]
    fn from_env_ignores_non_numeric_values() {
        // SAFETY: single-threaded test; cleaned up before returning.
        unsafe {
            std::env::set_var("CONUSAI_MAX_TOOLS_PER_TURN", "not_a_number");
            std::env::set_var("CONUSAI_MAX_INVOKES_PER_TURN", "also_bad");
        }
        let cfg = RouterQuotaConfig::from_env();
        unsafe {
            std::env::remove_var("CONUSAI_MAX_TOOLS_PER_TURN");
            std::env::remove_var("CONUSAI_MAX_INVOKES_PER_TURN");
        }
        // Falls back to defaults when parsing fails.
        assert_eq!(cfg.max_tools_per_turn, DEFAULT_MAX_TOOLS_PER_TURN);
        assert_eq!(cfg.max_invokes_per_turn, DEFAULT_MAX_INVOKES_PER_TURN);
    }

    // ── Middleware injects config into request extensions ─────────────────

    #[tokio::test]
    async fn middleware_injects_config_into_extensions() {
        // The handler reads the Arc<RouterQuotaConfig> from extensions.
        async fn handler(req: axum::extract::Request) -> (StatusCode, String) {
            let ext = req.extensions().get::<Arc<RouterQuotaConfig>>().cloned();
            match ext {
                Some(cfg) => (
                    StatusCode::OK,
                    format!(
                        "tools={},invokes={}",
                        cfg.max_tools_per_turn, cfg.max_invokes_per_turn
                    ),
                ),
                None => (StatusCode::INTERNAL_SERVER_ERROR, "missing config".into()),
            }
        }

        let quota_cfg = RouterQuotaConfig {
            max_tools_per_turn: 42,
            max_invokes_per_turn: 7,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            quota: None,
            upgrade_url: "/account/billing".into(),
        };

        let app = Router::new()
            .route("/test", get(handler))
            .layer(RouterQuotaLayer::new(quota_cfg));

        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert_eq!(body_str, "tools=42,invokes=7");
    }

    // ── Per-plan limit override ───────────────────────────────────────────

    /// When `PlanLimits` is in extensions, the middleware overrides the global
    /// env-var defaults with per-plan tool/invoke caps.
    #[tokio::test]
    async fn plan_limits_override_global_defaults() {
        use agent_core::context::tenant::PlanTier;

        async fn handler(req: axum::extract::Request) -> (StatusCode, String) {
            let cfg = req.extensions().get::<Arc<RouterQuotaConfig>>().cloned();
            match cfg {
                Some(c) => (StatusCode::OK, format!("tools={},invokes={}", c.max_tools_per_turn, c.max_invokes_per_turn)),
                None => (StatusCode::INTERNAL_SERVER_ERROR, "no config".into()),
            }
        }

        // Global defaults that should be overridden by Enterprise PlanLimits.
        let global_cfg = RouterQuotaConfig {
            max_tools_per_turn: 99,
            max_invokes_per_turn: 99,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            quota: None,
            upgrade_url: "/billing".into(),
        };

        let app = Router::new()
            .route("/test", get(handler))
            .layer(RouterQuotaLayer::new(global_cfg));

        // Inject Enterprise PlanLimits (50 tools, 25 invokes).
        let enterprise = PlanTier::Enterprise.limits();
        let req = axum::http::Request::builder()
            .uri("/test")
            .extension(enterprise)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "tools=50,invokes=25");
    }

    #[tokio::test]
    async fn free_plan_limits_restrict_tools() {
        use agent_core::context::tenant::PlanTier;

        async fn handler(req: axum::extract::Request) -> (StatusCode, String) {
            let cfg = req.extensions().get::<Arc<RouterQuotaConfig>>().cloned().unwrap();
            (StatusCode::OK, format!("tools={},invokes={}", cfg.max_tools_per_turn, cfg.max_invokes_per_turn))
        }

        let app = Router::new()
            .route("/test", get(handler))
            .layer(RouterQuotaLayer::new(RouterQuotaConfig::default()));

        let free = PlanTier::Free.limits();
        let req = axum::http::Request::builder()
            .uri("/test")
            .extension(free)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        // Free: 10 tools, 5 invokes — much less than the 25/10 global defaults.
        assert_eq!(std::str::from_utf8(&body).unwrap(), "tools=10,invokes=5");
    }
}
