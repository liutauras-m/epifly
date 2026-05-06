use agent_core::{WorkspaceIndexer, indexing::RealFsWatcher};
use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use jobs::JobSchedulerService;
use prometheus::Encoder;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer, ExposeHeaders};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;

mod capabilities;
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

/// Build a CORS layer that allows the configured web origin(s).
///
/// `WEB_ORIGIN` env → comma-separated origins (e.g. `https://app.conusai.com`).
/// Falls back to `http://localhost:3000` for local dev.
fn build_cors() -> CorsLayer {
    let raw = std::env::var("WEB_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());

    let origins: Vec<axum::http::HeaderValue> = raw
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::list([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-tenant-id"),
            axum::http::HeaderName::from_static("x-api-key"),
        ]))
        .expose_headers(ExposeHeaders::list([axum::http::HeaderName::from_static(
            "x-request-id",
        )]))
        .allow_credentials(true)
}

fn resolve_assets_dir() -> Result<PathBuf> {
    if let Ok(configured) = std::env::var("CONUSAI_UI_ASSETS") {
        let configured_path = PathBuf::from(configured);
        anyhow::ensure!(
            configured_path.is_dir(),
            "CONUSAI_UI_ASSETS does not point to an existing directory: {}",
            configured_path.display()
        );
        return Ok(configured_path);
    }

    // Resolve from common run layouts first, then fall back to crate-relative path.
    let candidates = [
        PathBuf::from("crates/agent-gateway/assets"),
        PathBuf::from("apps/backend/crates/agent-gateway/assets"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
    ];

    for candidate in candidates {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    anyhow::bail!(
        "Unable to resolve UI assets directory. Set CONUSAI_UI_ASSETS to the absolute assets path."
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    // Hold the guard until process exit — flushes OTLP spans + metrics on shutdown.
    let (_telemetry, prom_registry) = common::telemetry::init("agent-gateway", "info");
    let prom_registry = Arc::new(prom_registry);

    let state = Arc::new(AppState::from_env().await?);
    let loaded = state.registry.lock().unwrap().len();
    info!(capabilities = loaded, "capability registry loaded");

    // Register dynamic capabilities that depend on runtime services (e.g. JobExecutor).
    {
        use agent_core::tools::provider::CapabilityProvider;
        use capabilities::transcribe_video::TranscribeVideoCapability;
        let provider: Arc<dyn CapabilityProvider> = Arc::new(TranscribeVideoCapability::new(
            Arc::clone(&state.job_executor),
        ));
        let card = agent_core::tools::card::CapabilityCard::new(
            provider.manifest().clone(),
            std::path::PathBuf::from("runtime"),
        )
        .with_provider(Arc::clone(&provider));
        state.registry.lock().unwrap().register(card);
        info!("TranscribeVideoCapability registered");
    }

    // Validate that all LLM aliases resolve to registered providers.
    // Logs a warning on failure but does not abort startup (provider may be
    // temporarily unavailable at deploy time).
    if let Err(e) = agent_core::llm::verify_llm_providers(&state.llm).await {
        tracing::warn!(error = %e, "LLM registry verification failed");
    } else {
        info!("LLM registry verified");
    }

    // Start the cron scheduler in the background.
    // The `_scheduler` guard is kept alive for the process lifetime.
    let _scheduler = JobSchedulerService::start(&state.job_registry).await?;
    info!("job scheduler started");

    // Start the workspace file indexer if WORKSPACES_ROOT is configured.
    // Runs an initial index pass then watches for changes at configurable intervals.
    let _watcher = if let Ok(root) = std::env::var("WORKSPACES_ROOT") {
        if let Some(pool) = state.pool.clone() {
            let root_path = PathBuf::from(root);
            let indexer = Arc::new(WorkspaceIndexer::new(
                root_path.clone(),
                pool,
                Arc::clone(&state.embedding_service),
                Arc::clone(&state.vector_store),
            ));
            // Run the first indexing pass in the background; don't block startup.
            let idx_clone = Arc::clone(&indexer);
            tokio::spawn(async move {
                if let Err(e) = idx_clone.index_once().await {
                    tracing::warn!(error = %e, "initial workspace index pass failed");
                }
            });
            let watcher = RealFsWatcher::spawn(Arc::clone(&indexer));
            info!(root = %root_path.display(), "workspace indexer started");
            Some(watcher)
        } else {
            info!("WORKSPACES_ROOT set but no pool available — workspace indexer disabled");
            None
        }
    } else {
        info!("WORKSPACES_ROOT not set — workspace indexer disabled");
        None
    };

    let assets_dir = resolve_assets_dir()?;
    info!(assets_dir = %assets_dir.display(), "serving UI assets");

    let app = Router::new()
        .merge(routes::public_router())
        // Prometheus metrics — no auth required (restrict via network/proxy in prod)
        .route(
            "/metrics",
            get(metrics_handler).with_state(Arc::clone(&prom_registry)),
        )
        .merge(
            routes::protected_router()
                .layer(axum::middleware::from_fn(mw::plan::enforce_plan))
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::tenant::extract_tenant,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::api_key::extract_api_key,
                ))
                .layer(axum::middleware::from_fn(mw::trace::propagate_trace))
                .layer(axum::middleware::from_fn(mw::request_id::inject_request_id)),
        )
        .merge(ui::ui_router())
        .merge(
            routes::admin_router()
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::tenant::extract_tenant,
                ))
                .layer(axum::middleware::from_fn(mw::trace::propagate_trace))
                .layer(axum::middleware::from_fn(mw::request_id::inject_request_id)),
        )
        .nest_service("/assets", ServeDir::new(&assets_dir))
        .layer(build_cors())
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
