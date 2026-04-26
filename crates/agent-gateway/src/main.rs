use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use prometheus::Encoder;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;

mod mw;
mod routes;
mod state;
mod ui;

use state::AppState;

/// GET /metrics — Prometheus text exposition format.
async fn metrics_handler(State(registry): State<Arc<prometheus::Registry>>) -> impl IntoResponse {
    let mut buf = Vec::new();
    let encoder = prometheus::TextEncoder::new();
    if let Err(e) = encoder.encode(&registry.gather(), &mut buf) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("metrics encode error: {e}"),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        buf,
    )
        .into_response()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Hold the guard until process exit — flushes OTLP spans + metrics on shutdown.
    let (_telemetry, prom_registry) = common::telemetry::init("agent-gateway", "info");
    let prom_registry = Arc::new(prom_registry);

    let state = Arc::new(AppState::from_env()?);
    let loaded = state.registry.lock().unwrap().len();
    info!(capabilities = loaded, "capability registry loaded");

    let assets_dir =
        std::env::var("CONUSAI_UI_ASSETS").unwrap_or_else(|_| "crates/agent-gateway/assets".into());

    let app = Router::new()
        .merge(routes::public_router())
        // Prometheus metrics — no auth required (restrict via network/proxy in prod)
        .route(
            "/metrics",
            get(metrics_handler).with_state(Arc::clone(&prom_registry)),
        )
        .merge(
            routes::protected_router()
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::tenant::extract_tenant,
                ))
                .layer(axum::middleware::from_fn(mw::trace::propagate_trace)),
        )
        .merge(ui::ui_router())
        .nest_service("/assets", ServeDir::new(&assets_dir))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::clone(&state));

    let addr = format!(
        "{}:{}",
        std::env::var("CONUSAI_SERVER__HOST").unwrap_or_else(|_| "0.0.0.0".into()),
        std::env::var("CONUSAI_SERVER__PORT").unwrap_or_else(|_| "8080".into()),
    );

    info!("agent-gateway listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
