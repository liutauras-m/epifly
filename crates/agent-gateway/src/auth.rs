use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub async fn require_bearer(req: Request, next: Next) -> Response {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    if jwt_secret.is_empty() {
        // No secret configured — skip auth (dev mode)
        return next.run(req).await;
    }

    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(t) if t == jwt_secret => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    }
}
