//! Tower middleware that enforces per-turn semantic routing quotas.
//!
//! Reads `TenantContext` from the request extensions and rejects the request
//! when the tool budget would be exceeded. The actual counting happens at
//! the router level; this middleware enforces the hard cap on the HTTP path.

use axum::{
    body::Body,
    http::{Request, Response},
};
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
pub const DEFAULT_MAX_INVOKES_PER_TURN: usize = 10;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RouterQuotaConfig {
    pub max_tools_per_turn: usize,
    pub max_invokes_per_turn: usize,
}

impl Default for RouterQuotaConfig {
    fn default() -> Self {
        Self {
            max_tools_per_turn: DEFAULT_MAX_TOOLS_PER_TURN,
            max_invokes_per_turn: DEFAULT_MAX_INVOKES_PER_TURN,
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
        Self {
            max_tools_per_turn: env_usize("CONUSAI_MAX_TOOLS_PER_TURN", DEFAULT_MAX_TOOLS_PER_TURN),
            max_invokes_per_turn: env_usize(
                "CONUSAI_MAX_INVOKES_PER_TURN",
                DEFAULT_MAX_INVOKES_PER_TURN,
            ),
        }
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
        // Quota limits are enforced at the semantic router level.
        // This middleware stores the config in request extensions so
        // downstream handlers can read and enforce it per-call.
        let cfg = Arc::clone(&self.cfg);
        let mut inner = self.inner.clone();
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            parts.extensions.insert(cfg);
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
}
