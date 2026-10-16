use anyhow::Result;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use jobs::JobSchedulerService;
use prometheus::Encoder;
use std::sync::Arc;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer, ExposeHeaders};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod auth;
mod capabilities;
mod config;
mod metrics;
mod mw;
mod plans;
mod routes;
mod state;
#[cfg(test)]
mod tests;
mod ui;

use config::GatewayConfig;
use state::AppState;

/// `GET /metrics` — Prometheus text format. Not public: requires
/// `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>` (metrics also leave via OTLP).
async fn metrics_handler(
    State((state, registry)): State<(Arc<AppState>, Arc<prometheus::Registry>)>,
    headers: HeaderMap,
) -> axum::response::Response {
    if !auth::has_platform_token(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut buf = Vec::new();
    if let Err(e) = prometheus::TextEncoder::new().encode(&registry.gather(), &mut buf) {
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

/// CORS for the configured web origins. Credentials travel in the
/// `Authorization` header only — no cookies, so no credentialed CORS.
fn build_cors(origins: &[String]) -> CorsLayer {
    let origins: Vec<axum::http::HeaderValue> =
        origins.iter().filter_map(|s| s.parse().ok()).collect();
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
            axum::http::HeaderName::from_static("traceparent"),
        ]))
        .expose_headers(ExposeHeaders::list([axum::http::HeaderName::from_static(
            "x-request-id",
        )]))
}

/// Assemble the HTTP router. Every route outside the public and internal groups
/// passes `auth::authenticate`.
pub(crate) fn build_router(
    state: Arc<AppState>,
    prom_registry: Arc<prometheus::Registry>,
    web_origins: &[String],
) -> Router {
    let authenticated = |r: Router<Arc<AppState>>| {
        r.layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            auth::authenticate,
        ))
        .layer(axum::middleware::from_fn(mw::trace::propagate_trace))
        .layer(axum::middleware::from_fn(mw::request_id::inject_request_id))
    };

    let protected = routes::protected_router(state.quota.clone())
        .merge(ui::ui_router())
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            mw::meter::record_usage,
        ))
        .layer(axum::middleware::from_fn(mw::plan::enforce_plan));

    let mut app = Router::new()
        .merge(routes::public_router())
        .merge(authenticated(protected))
        .merge(authenticated(routes::admin_router()));
    if state.rustfs_webhook_secret.is_some() {
        app = app.merge(routes::internal_router());
    }

    app.route(
        "/metrics",
        get(metrics_handler).with_state((Arc::clone(&state), prom_registry)),
    )
    .layer(build_cors(web_origins))
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--dump-routes") {
        print!("{}", routes::dump_routes_markdown());
        return Ok(());
    }
    if std::env::args().any(|a| a == "--hash-api-key") {
        // Reads the key from stdin (never argv, which other processes can see) and
        // prints the BLAKE3 hash to put into `API_KEYS`.
        let mut key = String::new();
        std::io::stdin().read_line(&mut key)?;
        println!("{}", auth::hash_api_key(key.trim()));
        return Ok(());
    }
    if std::env::args().any(|a| a == "--dump-openapi") {
        println!("{}", routes::openapi_document());
        return Ok(());
    }

    // The whole environment contract is validated before anything starts.
    let cfg = match GatewayConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("agent-gateway: {e}");
            std::process::exit(2);
        }
    };

    let (_telemetry, prom_registry) = common::telemetry::init("agent-gateway", "info")?;
    let prom_registry = Arc::new(prom_registry);

    billing_core::metrics::register(&prom_registry);
    let rustfs_metrics = metrics::RustFsMetrics::register(&prom_registry);
    let router_metrics = metrics::RouterMetrics::register(&prom_registry);

    if cfg.mcp_allowlist.is_empty() {
        info!("CONUSAI_MCP_ALLOWED_HOSTS is empty — remote MCP endpoints are disabled");
    }

    let mut state = AppState::from_config(&cfg).await?;
    state.rustfs_metrics = Some(Arc::clone(&rustfs_metrics));
    state.router_metrics = Some(Arc::clone(&router_metrics));
    let state = Arc::new(state);
    info!(
        capabilities = state.registry.lock().unwrap().len(),
        "capability registry loaded"
    );
    rustfs_metrics.set_embedding_dims(
        state.embedding_service.model().name(),
        state.embedding_service.dims() as i64,
    );

    // Mirror the onboarding counters into Prometheus every 30 s.
    if let Some(svc) = state.onboarding.clone() {
        let m = Arc::clone(&rustfs_metrics);
        tokio::spawn(async move {
            let mut last_onboarding = 0u64;
            let mut last_marker_failed = 0u64;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
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
        });
    }

    if let Some(admin) = &state.rustfs_admin
        && let Err(e) = rustfs_admin::bootstrap_storage(admin, &cfg.bootstrap).await
    {
        warn!(error = %e, "RustFS bootstrap failed — storage may be degraded");
    }

    if let Err(e) = agent_core::llm::verify_llm_providers(&state.llm).await {
        warn!(error = %e, "LLM registry verification failed");
    }

    if let Some(billing) = &state.billing {
        match billing.ensure_plans(&state.plan_catalog).await {
            Ok(()) => info!("Lago plan catalog seeded"),
            Err(e) => warn!(error = %e, "ensure_plans failed — Lago may not be reachable yet"),
        }
    }

    let _scheduler = JobSchedulerService::start(&state.job_registry).await?;
    info!("job scheduler started");

    if state.rustfs_webhook_secret.is_some() {
        info!("workspace indexer: event-driven via POST /internal/rustfs/events");
    } else {
        info!("RustFS bucket notifications not configured — event-driven indexing is off");
    }

    let app = build_router(Arc::clone(&state), prom_registry, &cfg.web_origins);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], cfg.port));
    info!(%addr, "agent-gateway listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("shutdown signal received");
}
