//! Multi-vector session extractor for Axum handlers and middleware.
//!
//! Auth vectors in priority order (documented here as the single source of truth):
//!
//!   1. `conusai_session` cookie  — web app, same-origin (browser sends automatically).
//!   2. `X-Session-Token` header  — Tauri WKWebView. WKWebView cannot attach Secure
//!      cookies to cross-origin plain-HTTP requests, so `apps/browser-shell/src/lib/sdk.ts`
//!      injects the HMAC token as a request header instead.
//!
//! Bearer JWT (`Authorization: Bearer`) is handled separately in `mw/tenant.rs` for
//! Zitadel OIDC tokens and API keys.

use super::verifier::{SESSION_HEADER, SessionUser, verify};
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, request::Parts},
    response::{IntoResponse, Response},
};

/// Extract a verified `SessionUser` from the request's `HeaderMap`.
///
/// Tries cookie first (dominant web path), then `X-Session-Token` header (Tauri WKWebView).
/// Returns `None` if neither vector yields a valid, unexpired session.
///
/// Used directly by `mw/tenant.rs` so both `/ui/*` and `/v1/*` share identical logic.
pub fn extract_from_headers(headers: &HeaderMap) -> Option<SessionUser> {
    from_cookie(headers).or_else(|| from_header(headers))
}

fn from_cookie(headers: &HeaderMap) -> Option<SessionUser> {
    let cookie_str = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    cookie_str
        .split(';')
        .find_map(|c| c.trim().strip_prefix("conusai_session=").and_then(verify))
}

fn from_header(headers: &HeaderMap) -> Option<SessionUser> {
    headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| verify(s.trim()))
}

/// Axum extractor — used by `/ui/*` handlers that need the session user directly.
///
/// Implements the same cookie → header priority as `extract_from_headers`.
/// Returns a 401 response if neither vector is valid.
impl<S: Send + Sync> FromRequestParts<S> for SessionUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        extract_from_headers(&parts.headers)
            .ok_or_else(|| axum::http::StatusCode::UNAUTHORIZED.into_response())
    }
}
