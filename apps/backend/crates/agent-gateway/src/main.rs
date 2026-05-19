use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use jobs::JobSchedulerService;
use prometheus::Encoder;
use std::sync::Arc;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer, ExposeHeaders};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod auth;
mod capabilities;
mod metrics;
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
fn build_cors() -> CorsLayer {
    let raw = std::env::var("WEB_ORIGIN").unwrap_or_else(|_| {
        "http://localhost:3000,http://localhost:5173,https://tauri.localhost,tauri://localhost"
            .into()
    });

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
            axum::http::HeaderName::from_static("x-session-token"),
        ]))
        .expose_headers(ExposeHeaders::list([axum::http::HeaderName::from_static(
            "x-request-id",
        )]))
        .allow_credentials(true)
}

#[tokio::main]
async fn main() -> Result<()> {
    let (_telemetry, prom_registry) = common::telemetry::init("agent-gateway", "info");
    let prom_registry = Arc::new(prom_registry);

    // Register billing/quota metrics + RustFS metrics.
    billing_core::metrics::register(&prom_registry);
    let rustfs_metrics = metrics::RustFsMetrics::register(&prom_registry);

    let state = Arc::new(AppState::from_env().await?);
    let loaded = state.registry.lock().unwrap().len();
    info!(capabilities = loaded, "capability registry loaded");

    // ── Declarative RustFS bootstrap ──────────────────────────────────────
    if let Some(ref admin) = state.rustfs_admin {
        let cfg = rustfs_admin::BootstrapConfig::from_env();
        if let Err(e) = rustfs_admin::bootstrap_storage(admin, &cfg).await {
            warn!(error = %e, "RustFS bootstrap failed — storage may be degraded");
        }
    } else {
        info!("RustFS admin not configured — skipping declarative bootstrap");
    }

    // ── Capability-spec realtime hot-reload ───────────────────────────────
    if let Some(spec_factory) = state.capability_spec_factory.clone() {
        let realtime = Arc::clone(&state.realtime_service);
        let registry = Arc::clone(&state.registry);
        tokio::spawn(async move {
            let mut rx = realtime.subscribe_capability_spec_changes().await;
            while let Some((namespace, tool_name)) = rx.recv().await {
                if let Err(e) = spec_factory
                    .reload_one(&registry, &namespace, &tool_name)
                    .await
                {
                    warn!(error = %e, namespace, tool_name, "capability-spec hot-reload failed");
                }
            }
        });
        info!("capability-spec realtime hot-reload listener started");
    }

    // ── Register dynamic capabilities ────────────────────────────────────
    {
        use agent_core::capabilities::provider::CapabilityProvider;
        use capabilities::transcribe_video::TranscribeVideoCapability;
        let provider: Arc<dyn CapabilityProvider> = Arc::new(TranscribeVideoCapability::new(
            Arc::clone(&state.job_executor),
        ));
        let card = agent_core::capabilities::card::CapabilityCard::new(
            provider.manifest().clone(),
            std::path::PathBuf::from("runtime"),
        )
        .with_provider(Arc::clone(&provider));
        state.registry.lock().unwrap().register(card);
        info!("TranscribeVideoCapability registered");
    }

    // ── LLM registry verification ────────────────────────────────────────
    if let Err(e) = agent_core::llm::verify_llm_providers(&state.llm).await {
        tracing::warn!(error = %e, "LLM registry verification failed");
    } else {
        info!("LLM registry verified");
    }

    // ── Lago plan catalog seeding ────────────────────────────────────────
    if let Some(ref billing) = state.billing {
        use billing_core::provider::BillingProvider as _;
        if let Err(e) = billing.ensure_plans(&state.plan_catalog).await {
            tracing::warn!(error = %e, "ensure_plans failed — Lago may not be reachable yet");
        } else {
            info!("Lago plan catalog seeded");
        }
    }

    // ── Start cron scheduler ─────────────────────────────────────────────
    let _scheduler = JobSchedulerService::start(&state.job_registry).await?;
    info!("job scheduler started");

    // ── Event-driven workspace indexer ───────────────────────────────────
    // The old polling RealFsWatcher is removed. Indexing is now driven by
    // RustFS bucket notifications → POST /internal/rustfs/events.
    if std::env::var("RUSTFS_NOTIFICATIONS").as_deref() == Ok("off") {
        warn!("RUSTFS_NOTIFICATIONS=off — workspace indexing is disabled");
    } else {
        info!("workspace indexer: event-driven via /internal/rustfs/events webhook");
    }

    // ── Build Axum router ────────────────────────────────────────────────
    let app = Router::new()
        .merge(routes::public_router())
        .route(
            "/metrics",
            get(metrics_handler).with_state(Arc::clone(&prom_registry)),
        )
        // Internal routes — not authenticated (restrict by network in prod)
        .merge(routes::internal_router().with_state(Arc::clone(&state)))
        .merge(
            routes::protected_router(state.quota.clone())
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::meter::record_usage,
                ))
                .layer(axum::middleware::from_fn(mw::plan::enforce_plan))
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::identity::extract_identity,
                ))
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
