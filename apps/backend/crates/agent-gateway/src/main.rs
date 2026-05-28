use anyhow::Result;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use jobs::JobSchedulerService;
use prometheus::Encoder;
use std::sync::Arc;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer, ExposeHeaders};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

// The library crate (lib.rs) publishes all modules so integration tests can import them.
// main.rs re-uses those modules via the library.
use agent_gateway::{metrics, mw, routes, state, ui};

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
    // Lightweight flag — no clap dep needed for a single flag.
    if std::env::args().any(|a| a == "--dump-routes") {
        print!("{}", routes::dump_routes_markdown());
        return Ok(());
    }
    if std::env::args().any(|a| a == "--validate-config") {
        AppState::validate_env_contracts()?;
        println!("validate-config: OK");
        return Ok(());
    }

    let (_telemetry, prom_registry) = common::telemetry::init("agent-gateway", "info");
    let prom_registry = Arc::new(prom_registry);

    // Register billing/quota metrics + RustFS metrics + router metrics.
    billing_core::metrics::register(&prom_registry);
    let rustfs_metrics = metrics::RustFsMetrics::register(&prom_registry);
    let router_metrics = metrics::RouterMetrics::register(&prom_registry);

    let mut state = AppState::from_env().await?;
    state.rustfs_metrics = Some(Arc::clone(&rustfs_metrics));
    state.router_metrics = Some(Arc::clone(&router_metrics));
    let state = Arc::new(state);
    let loaded = state.registry.read().len();
    info!(capabilities = loaded, "capability registry loaded");

    // ── Set embedding_dims gauge at startup ──────────────────────────────
    {
        let model = agent_core::indexing::embedding_service::EmbeddingModel::from_env();
        rustfs_metrics.set_embedding_dims(model.name(), model.dims() as i64);
    }

    // ── Sync atomic counters into Prometheus (every 30 s) ────────────────
    // TenantStorageFactory, TenantOnboardingService, and ZitadelProvider use
    // AtomicU64 counters that we mirror into Prometheus IntCounterVec here.
    if let Some(m) = state.rustfs_metrics.clone() {
        let factory = state.tenant_storage.clone();
        let onboarding = state.onboarding.clone();
        let zitadel_stats = state.zitadel_cache_stats.clone();
        tokio::spawn(async move {
            let mut last_fallback = 0u64;
            let mut last_onboarding = 0u64;
            let mut last_marker_failed = 0u64;
            let mut last_z_hits = 0u64;
            let mut last_z_misses = 0u64;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                if let Some(ref f) = factory {
                    let cur = f.fallback_count.load(std::sync::atomic::Ordering::Relaxed);
                    if cur > last_fallback {
                        m.record_storage_fallback("dev_fallback");
                        last_fallback = cur;
                    }
                }

                if let Some(ref svc) = onboarding {
                    let cur = svc
                        .onboarding_total
                        .load(std::sync::atomic::Ordering::Relaxed);
                    if cur > last_onboarding {
                        m.record_onboarding("normal");
                        last_onboarding = cur;
                    }

                    let cur = svc.marker_failed.load(std::sync::atomic::Ordering::Relaxed);
                    if cur > last_marker_failed {
                        m.record_onboarding_marker_failed();
                        last_marker_failed = cur;
                    }
                }

                if let Some(ref zs) = zitadel_stats {
                    let hits = zs.hits();
                    if hits > last_z_hits {
                        for _ in 0..(hits - last_z_hits) {
                            m.record_zitadel_cache_hit();
                        }
                        last_z_hits = hits;
                    }
                    let misses = zs.misses();
                    if misses > last_z_misses {
                        for _ in 0..(misses - last_z_misses) {
                            m.record_zitadel_cache_miss();
                        }
                        last_z_misses = misses;
                    }
                }
            }
        });
    }

    // ── Declarative RustFS bootstrap ──────────────────────────────────────
    if let Some(ref admin) = state.rustfs_admin {
        let cfg = rustfs_admin::BootstrapConfig::from_env();
        if let Err(e) = rustfs_admin::bootstrap_storage(admin, &cfg).await {
            warn!(error = %e, "RustFS bootstrap failed — storage may be degraded");
        }
    } else {
        info!("RustFS admin not configured — skipping declarative bootstrap");
    }

    // ── LLM registry verification ────────────────────────────────────────
    if let Err(e) = agent_core::llm::verify_llm_providers(&state.llm).await {
        tracing::warn!(error = %e, "LLM registry verification failed");
    } else {
        info!("LLM registry verified");
    }

    // ── Lago plan catalog seeding ────────────────────────────────────────
    if let Some(ref billing) = state.billing {
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
                .layer(axum::middleware::from_fn(mw::trace::propagate_trace)),
        )
        .merge(ui::ui_router())
        .merge(
            routes::admin_router()
                .layer(axum::middleware::from_fn_with_state(
                    Arc::clone(&state),
                    mw::tenant::extract_tenant,
                ))
                .layer(axum::middleware::from_fn(mw::trace::propagate_trace)),
        )
        .layer(build_cors())
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(mw::request_id::inject_request_id))
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
