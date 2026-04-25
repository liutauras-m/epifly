use anyhow::Result;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

mod mw;
mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    common::telemetry::init("info");

    let state = Arc::new(AppState::from_env()?);
    let loaded = state.registry.lock().unwrap().len();
    info!(capabilities = loaded, "capability registry loaded");

    let app = Router::new()
        .merge(routes::public_router())
        .merge(
            routes::protected_router().layer(axum::middleware::from_fn_with_state(
                Arc::clone(&state),
                mw::tenant::extract_tenant,
            )),
        )
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
