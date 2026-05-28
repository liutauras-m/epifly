//! Rate-limiting middleware for auth endpoints.
//!
//! Applied to `/v1/auth/*` routes in the public router.
//! Keyed by IP prefix; 10 requests per 60-second window per key.

use crate::mw::rate_limit::ip_key;
use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

const AUTH_RATE_LIMIT_RPM: u32 = 10;

pub async fn auth_rate_limit(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = ip_key(req.headers());
    if !state
        .rate_limiter
        .check(&format!("auth:{ip}"), AUTH_RATE_LIMIT_RPM)
    {
        return (StatusCode::TOO_MANY_REQUESTS, "rate_limited").into_response();
    }
    next.run(req).await
}
