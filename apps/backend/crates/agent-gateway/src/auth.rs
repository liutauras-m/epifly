//! Request authentication — the one place a `TenantContext` is built.
//!
//! Every protected route requires `Authorization: Bearer <token>`, where the token is
//! - a hashed **API key** from `API_KEYS` (machine clients), or
//! - a **Zitadel access token** (JWT verified via JWKS, or opaque via introspection).
//!
//! The tenant is the API key's tenant or the token's Zitadel organization. The plan
//! tier is resolved server-side by [`crate::plans::PlanResolver`]; nothing a client
//! sends decides it. There are no cookies, session headers, tenant headers or
//! credential-less modes.

use crate::state::AppState;
use agent_core::{AuthError, TenantContext, UserRole};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use common::secret::{bearer_token, constant_time_eq};
use std::sync::Arc;
use tracing::warn;

/// Request extension carrying the authenticated tenant context.
#[derive(Clone)]
pub struct ResolvedTenant(pub TenantContext);

/// One machine-client key: `API_KEYS=<blake3-hex>:<tenant_id>:<label>,…`.
/// Only the BLAKE3 hash of the key is configured, never the key itself.
#[derive(Clone)]
pub struct ApiKeyEntry {
    pub hash: [u8; 32],
    pub tenant_id: String,
    pub label: String,
}

impl std::fmt::Debug for ApiKeyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyEntry")
            .field("tenant_id", &self.tenant_id)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// Parse `API_KEYS`. Any malformed entry fails the whole value.
pub fn parse_api_keys(raw: &str) -> Result<Vec<ApiKeyEntry>, String> {
    let mut out = Vec::new();
    for (i, entry) in raw
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .enumerate()
    {
        let parts: Vec<&str> = entry.split(':').collect();
        let [hash_hex, tenant_id, label] = parts.as_slice() else {
            return Err(format!(
                "entry {} must be <blake3-hex>:<tenant_id>:<label>",
                i + 1
            ));
        };
        let bytes = hex::decode(hash_hex)
            .ok()
            .filter(|b| b.len() == 32)
            .ok_or_else(|| format!("entry {}: hash must be 64 hex characters", i + 1))?;
        if tenant_id.is_empty() {
            return Err(format!("entry {}: tenant_id is empty", i + 1));
        }
        if label.is_empty()
            || !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("entry {}: label must be [A-Za-z0-9_-]+", i + 1));
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        out.push(ApiKeyEntry {
            hash,
            tenant_id: tenant_id.to_string(),
            label: label.to_string(),
        });
    }
    Ok(out)
}

/// BLAKE3 hex of a raw API key (what goes into `API_KEYS`).
pub fn hash_api_key(raw_key: &str) -> String {
    blake3::hash(raw_key.as_bytes()).to_hex().to_string()
}

fn match_api_key<'a>(keys: &'a [ApiKeyEntry], token: &str) -> Option<&'a ApiKeyEntry> {
    let presented = blake3::hash(token.as_bytes());
    // Compare against every entry so timing does not reveal which one matched.
    let mut found = None;
    for k in keys {
        if constant_time_eq(presented.as_bytes(), &k.hash) {
            found = Some(k);
        }
    }
    found
}

fn unauthorized(msg: &str) -> Response {
    let mut resp = HttpError::auth(msg).into_response();
    resp.headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    resp
}

/// Resolve the caller from the bearer token. `Err` is the response to send.
async fn resolve(state: &AppState, headers: &HeaderMap) -> Result<TenantContext, Box<Response>> {
    let token =
        bearer_token(headers).ok_or_else(|| Box::new(unauthorized("authentication required")))?;

    let (tenant_id, user_id, role) = if let Some(key) = match_api_key(&state.api_keys, token) {
        (
            key.tenant_id.clone(),
            format!("api-key:{}", key.label),
            UserRole::User,
        )
    } else {
        match state.verifier.verify(token).await {
            Ok(identity) => (
                identity.tenant_id.to_string(),
                identity.user_id,
                identity.role,
            ),
            Err(AuthError::InvalidToken(reason)) => {
                warn!(%reason, "bearer token rejected");
                return Err(Box::new(unauthorized("invalid or expired token")));
            }
            Err(AuthError::Unavailable(reason)) => {
                warn!(%reason, "identity provider unavailable");
                return Err(Box::new(
                    HttpError::unavailable("identity provider unavailable").into_response(),
                ));
            }
        }
    };

    let plan = state.plans.resolve(&tenant_id).await;
    let mut ctx = TenantContext::new(tenant_id, Some(user_id), plan);
    ctx.role = role;
    Ok(ctx)
}

/// Middleware for every protected route group.
pub async fn authenticate(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    match resolve(&state, req.headers()).await {
        Ok(ctx) => {
            req.extensions_mut().insert(ResolvedTenant(ctx));
            next.run(req).await
        }
        Err(resp) => *resp,
    }
}

/// Middleware for `/admin/*` (layered inside [`authenticate`]): `super_admin` only.
pub async fn require_super_admin(req: Request, next: Next) -> Response {
    match req.extensions().get::<ResolvedTenant>() {
        Some(t) if t.0.role == UserRole::SuperAdmin => next.run(req).await,
        Some(_) => HttpError::forbidden("super_admin role required").into_response(),
        None => unauthorized("authentication required"),
    }
}

/// True when the request carries `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>`.
pub fn has_platform_token(state: &AppState, headers: &HeaderMap) -> bool {
    bearer_token(headers)
        .is_some_and(|t| constant_time_eq(t.as_bytes(), state.platform_admin_token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_keys() {
        let h = hash_api_key("secret-key");
        let keys = parse_api_keys(&format!("{h}:org-1:ci-bot, {h}:org-2:etl")).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].tenant_id, "org-1");
        assert_eq!(keys[1].label, "etl");
        assert!(match_api_key(&keys, "secret-key").is_some());
        assert!(match_api_key(&keys, "other").is_none());
    }

    #[test]
    fn rejects_malformed_keys() {
        let h = hash_api_key("k");
        assert!(parse_api_keys("nothex:org:label").is_err());
        assert!(parse_api_keys(&format!("{h}:org")).is_err());
        assert!(parse_api_keys(&format!("{h}::label")).is_err());
        assert!(parse_api_keys(&format!("{h}:org:bad label")).is_err());
        // The old `<hash>:<tenant>:<plan>:<extra>` shape is not accepted.
        assert!(parse_api_keys(&format!("{h}:org:pro:x")).is_err());
    }

    #[test]
    fn debug_hides_hash() {
        let h = hash_api_key("k");
        let keys = parse_api_keys(&format!("{h}:org:label")).unwrap();
        assert!(!format!("{:?}", keys[0]).contains(&h));
    }
}
