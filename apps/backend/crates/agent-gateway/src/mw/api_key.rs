/// API Key authentication middleware.
///
/// Reads `X-API-Key` header; if present, validates it against the `API_KEYS` env var
/// before JWT auth is attempted. This runs *inside* the tenant middleware so if
/// a valid API key is found, the tenant is set directly without needing a JWT.
///
/// `API_KEYS` format: comma-separated `<blake3_hex>:<tenant_id>:<plan>` tuples.
/// Only the BLAKE3 hash is stored — never the raw key.
///
/// Example:
///   API_KEYS=abc123hash:tenant1:pro,def456hash:tenant2:enterprise
use agent_core::{PlanTier, TenantContext};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use std::sync::Arc;
use tracing::warn;

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct ApiKeyEntry {
    pub hash_hex: String,
    pub tenant_id: String,
    pub plan: PlanTier,
}

/// Parse `API_KEYS` env var into a list of `ApiKeyEntry`.
pub fn parse_api_keys(raw: &str) -> Vec<ApiKeyEntry> {
    raw.split(',')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.trim().splitn(3, ':').collect();
            if parts.len() != 3 {
                return None;
            }
            let plan = match parts[2].to_lowercase().as_str() {
                "pro" => PlanTier::Pro,
                "enterprise" => PlanTier::Enterprise,
                _ => PlanTier::Free,
            };
            Some(ApiKeyEntry {
                hash_hex: parts[0].to_string(),
                tenant_id: parts[1].to_string(),
                plan,
            })
        })
        .collect()
}

/// Hash an API key with BLAKE3 and return the hex string.
pub fn hash_api_key(raw_key: &str) -> String {
    let hash = blake3::hash(raw_key.as_bytes());
    hash.to_hex().to_string()
}

/// Middleware: check `X-API-Key` header and resolve tenant if valid.
/// If `X-API-Key` is absent, falls through to the next middleware (JWT auth).
/// If `X-API-Key` is present but invalid, rejects immediately with 401.
///
/// API key entries are read from `state.api_keys` (populated at startup from the
/// `API_KEYS` env var) — no per-request env lookup.
pub async fn extract_api_key(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let raw_key = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let Some(key) = raw_key else {
        // No API key header — let JWT middleware handle auth
        return next.run(req).await;
    };

    if state.api_keys.is_empty() {
        warn!("no API keys configured but X-API-Key header provided — rejecting");
        return HttpError::auth("API key authentication not configured").into_response();
    }

    let key_hash = hash_api_key(&key);
    let matched = state.api_keys.iter().find(|e| e.hash_hex == key_hash);

    match matched {
        Some(entry) => {
            let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());
            let tenant = TenantContext::new(
                entry.tenant_id.as_str(),
                None::<&str>,
                entry.plan.clone(),
                workspace_root,
            );
            req.extensions_mut().insert(ResolvedTenant(tenant));
            next.run(req).await
        }
        None => {
            warn!("invalid API key presented");
            HttpError::auth("invalid API key").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use tower::ServiceExt;

    /// Build an `AppState` with specific API key entries — no env mutation needed.
    fn state_with_api_keys(entries: Vec<ApiKeyEntry>) -> Arc<crate::state::AppState> {
        let mut state =
            crate::state::AppState::with_in_memory_stores().expect("in-memory AppState");
        state.api_keys = entries;
        Arc::new(state)
    }

    #[tokio::test]
    async fn valid_api_key_maps_to_expected_tenant() {
        let raw_key = "super-secret-key";
        let state = state_with_api_keys(vec![ApiKeyEntry {
            hash_hex: hash_api_key(raw_key),
            tenant_id: "tenant-abc".into(),
            plan: PlanTier::Pro,
        }]);

        async fn handler(req: Request) -> (StatusCode, String) {
            match req.extensions().get::<ResolvedTenant>().cloned() {
                Some(ResolvedTenant(ctx)) => (StatusCode::OK, ctx.tenant_id.to_string()),
                None => (StatusCode::INTERNAL_SERVER_ERROR, "missing tenant".into()),
            }
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_api_key))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("x-api-key", raw_key)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "tenant-abc");
    }

    #[tokio::test]
    async fn invalid_api_key_is_rejected() {
        let state = state_with_api_keys(vec![ApiKeyEntry {
            hash_hex: hash_api_key("known-good-key"),
            tenant_id: "tenant-a".into(),
            plan: PlanTier::Free,
        }]);

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }

        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_api_key))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("x-api-key", "wrong-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_api_key_header_falls_through() {
        // No API keys configured; no X-API-Key header → falls through to the next layer.
        let state = state_with_api_keys(vec![]);

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }

        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_api_key))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_header_without_configuration_is_rejected() {
        // X-API-Key provided but state has no configured keys → 401.
        let state = state_with_api_keys(vec![]);

        async fn ok_handler() -> StatusCode {
            StatusCode::OK
        }

        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_api_key))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("x-api-key", "some-key")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
