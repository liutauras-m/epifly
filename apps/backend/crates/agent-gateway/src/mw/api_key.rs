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
pub async fn extract_api_key(
    State(_state): State<Arc<AppState>>,
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

    let api_keys_raw = std::env::var("API_KEYS").unwrap_or_default();
    if api_keys_raw.is_empty() {
        warn!("API_KEYS env var not set but X-API-Key header provided — rejecting");
        return HttpError::auth("API key authentication not configured").into_response();
    }

    let entries = parse_api_keys(&api_keys_raw);
    let key_hash = hash_api_key(&key);

    let matched = entries.iter().find(|e| e.hash_hex == key_hash);

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
