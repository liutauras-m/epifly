use crate::state::AppState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, extract::State};
use serde_json::json;
use std::sync::Arc;

/// Top-level `/health`. Reports embedding service + router readiness so misconfigured
/// deploys are obvious from a single curl, per Phase 1.6.4 of `docs/plan.md`.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = Value),
        (status = 503, description = "Service is degraded (embeddings or router unavailable)", body = Value),
    ),
    tag = "health",
)]
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cap_count = state.registry.lock().unwrap().len();
    let embeddings_ok = state.embedding_service.embed_query("ok").await.is_ok();
    let router_ok = embeddings_ok && cap_count > 0;
    let overall_ok = embeddings_ok && router_ok;

    let body = json!({
        "status": if overall_ok { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": cap_count,
        "embeddings": if embeddings_ok { "ok" } else { "fail" },
        "router": if router_ok { "ok" } else { "fail" },
        "registry_capabilities": cap_count,
    });

    let status = if overall_ok {
        StatusCode::OK
    } else {
        // 503 lets load balancers / orchestrators evict an unhealthy gateway.
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}

/// `GET /healthz/embeddings` — dedicated readiness probe for the embedding service.
///
/// Returns `200 { status: "ok", model, dims }` once the embedding backend can
/// produce a vector for the query "ok". Returns `503` while the model is still
/// loading or when `--features local-embeddings` was not compiled in. Used by
/// `start.sh` to gate on readiness before declaring the gateway "up" (Phase 1.3).
pub async fn embeddings_ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.embedding_service.embed_query("ok").await {
        Ok(vec) => {
            let model = state.embedding_service.model().name();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "model": model,
                    "dims": vec.len(),
                })),
            )
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "fail",
                "error": e.to_string(),
            })),
        ),
    }
}
