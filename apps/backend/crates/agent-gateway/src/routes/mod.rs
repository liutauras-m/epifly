use crate::mw::RouterQuotaLayer;
use crate::state::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

mod admin_capabilities;
pub(crate) mod admin_devices;
mod admin_jobs;
pub mod agent;
mod audit;
pub mod auth;
pub mod billing;
pub mod billing_webhook;
mod capabilities;
pub mod chat;
mod files;
mod health;
pub(crate) mod internal;
mod mcp;
pub mod realtime;
mod search;
mod shells;
mod tasks;
mod threads;
mod uploads;
mod workspaces;

/// Adds security scheme definitions to the generated OpenAPI spec.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
        components.add_security_scheme(
            "api_key_auth",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
        );
        components.add_security_scheme(
            "cookie_auth",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("conusai_session"))),
        );
    }
}

/// Assembled OpenAPI document (generated at startup).
#[derive(OpenApi)]
#[openapi(
    info(
        title = "ConusAI Platform API",
        version = "0.1.0",
        description = "OpenAI-compatible multitenant AI agent API"
    ),
    modifiers(&SecurityAddon),
    components(
        schemas(
            common::error::ErrorEnvelope,
            common::error::ApiErrorBody,
            common::error::ApiErrorKind,
            admin_devices::IssueDeviceRequest,
            admin_devices::IssueDeviceResponse,
            admin_devices::DeviceSummary,
        )
    ),
    paths(
        auth::login,
        health::health,
        chat::completions,
        agent::agent_completions,
        mcp::dispatch,
        capabilities::list_capabilities,
        search::search,
        audit::list_audit,
        workspaces::create,
        workspaces::tree,
        workspaces::search,
        workspaces::get_node,
        workspaces::delete_node,
        workspaces::get_content,
        workspaces::patch_content,
        workspaces::move_node,
        workspaces::share_node,
        workspaces::unshare_node,
        admin_devices::issue_device,
        admin_devices::list_devices,
        admin_devices::revoke_device,
    ),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "chat", description = "OpenAI-compatible chat completions"),
        (name = "agent", description = "Thread-aware agent completions with tool calling"),
        (name = "capabilities", description = "Tool capability registry"),
        (name = "mcp", description = "MCP JSON-RPC 2.0 tool protocol"),
        (name = "workspaces", description = "Hierarchical workspace management"),
        (name = "audit", description = "Audit log"),
        (name = "files", description = "File storage — presigned URL based"),
        (name = "uploads", description = "Multipart upload for large files"),
        (name = "admin", description = "Platform administration (device tokens, etc.)"),
    ),
)]
pub struct ApiDoc;

/// Routes that require no auth (health probe, auth, OpenAPI).
/// Note: the old `GET /v1/files/{token}` UUID download shim is REMOVED.
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        .route("/login", get(auth::login_page))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/legacy/login", post(auth::login))
        // Lago billing webhooks — signature verified inside handler.
        .route("/v1/billing/webhooks", post(billing_webhook::handle_webhook))
        // Self-registration for external capability services.
        .route(
            "/admin/capabilities/register",
            post(admin_capabilities::register_capability),
        )
        // OpenAPI spec + Swagger UI
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
}

/// Super-admin only routes (JWT-protected with role=super_admin).
pub fn admin_router() -> Router<Arc<AppState>> {
    use crate::mw::admin::require_super_admin_jwt;
    use axum::middleware;
    Router::new()
        .route("/admin/capabilities", get(admin_capabilities::list))
        .route("/admin/capabilities", post(admin_capabilities::create))
        .route(
            "/admin/capabilities/reload",
            post(admin_capabilities::reload_all),
        )
        .route(
            "/admin/capabilities/validate",
            post(admin_capabilities::validate),
        )
        .route(
            "/admin/capabilities/test",
            post(admin_capabilities::test_invoke),
        )
        .route(
            "/admin/capabilities/{name}",
            get(admin_capabilities::get_one),
        )
        .route(
            "/admin/capabilities/{name}/manifest",
            get(admin_capabilities::get_manifest),
        )
        .route(
            "/admin/capabilities/{name}",
            axum::routing::patch(admin_capabilities::update),
        )
        .route(
            "/admin/capabilities/{name}/enabled",
            axum::routing::patch(admin_capabilities::set_enabled),
        )
        .route(
            "/admin/capabilities/{name}",
            axum::routing::delete(admin_capabilities::delete_one),
        )
        .route(
            "/admin/capabilities/{name}/reload",
            post(admin_capabilities::reload_one),
        )
        // Dynamic prompt management
        .route(
            "/admin/capabilities/{name}/prompt",
            put(admin_capabilities::upsert_prompt).get(admin_capabilities::get_prompt),
        )
        .route(
            "/admin/capabilities/{name}/prompt/versions",
            get(admin_capabilities::list_prompt_versions),
        )
        // Namespace browser
        .route(
            "/admin/capabilities/namespaces",
            get(admin_capabilities::list_namespaces),
        )
        // Job management
        .route("/admin/jobs", get(admin_jobs::list_jobs))
        .route("/admin/jobs/{name}", get(admin_jobs::get_job))
        .route("/admin/jobs/{name}/run", post(admin_jobs::run_now))
        .route("/admin/tasks", get(admin_jobs::list_tasks))
        // ── Device tokens (browser-shell clients) ──────────────────────────
        .route("/admin/devices", post(admin_devices::issue_device))
        .route("/admin/devices", get(admin_devices::list_devices))
        .route("/admin/devices/{id}", delete(admin_devices::revoke_device))
        // ── Admin billing ────────────────────────────────────────────────────
        .route("/admin/billing/credits", post(billing::admin_add_credits))
        .route("/admin/billing/cancel/{tenant_id}", post(billing::admin_cancel_subscription))
        .route("/admin/billing/dashboard", get(billing::admin_billing_dashboard))
        .layer(middleware::from_fn(require_super_admin_jwt))
}

/// Internal routes — not exposed externally (mount behind a firewall or IP allowlist in prod).
pub fn internal_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/internal/rustfs/events", post(internal::rustfs_events))
}

/// Routes protected by the tenant middleware.
pub fn protected_router(
    quota: Option<std::sync::Arc<billing_core::quota::QuotaChecker>>,
) -> Router<Arc<AppState>> {
    let quota_cfg = {
        let mut cfg = crate::mw::router_quota::RouterQuotaConfig::from_env();
        if let Some(q) = quota {
            cfg = cfg.with_quota(q);
        }
        cfg
    };
    Router::new()
        // OpenAI-compatible chat
        .route("/v1/chat/completions", post(chat::completions))
        // Agent with tool calling + optional thread memory
        .route("/v1/agent/completions", post(agent::agent_completions))
        .route_layer(RouterQuotaLayer::new(quota_cfg))
        // Tool registry
        .route("/v1/capabilities", get(capabilities::list_capabilities))
        // Semantic capability search
        .route("/v1/capabilities/search", get(search::search))
        // MCP JSON-RPC 2.0
        .route("/mcp", post(mcp::dispatch))
        // ── File storage — presigned URL based (no proxy download) ───────────
        .route("/v1/files/upload-url", post(files::presign_upload))
        .route("/v1/files/download-url", get(files::presign_download))
        // ── Multipart upload for large files ────────────────────────────────
        .route("/v1/uploads/initiate", post(uploads::initiate))
        .route("/v1/uploads/{upload_id}/parts/{n}/presign", post(uploads::presign_part))
        .route("/v1/uploads/{upload_id}/complete", post(uploads::complete))
        .route("/v1/uploads/{upload_id}/abort", post(uploads::abort))
        // ── Audit log ──────────────────────────────────────────────────────
        .route("/v1/audit", get(audit::list_audit))
        // ── Workspace ──────────────────────────────────────────────────────
        .route("/v1/workspaces", post(workspaces::create))
        .route("/v1/workspaces/tree", get(workspaces::tree))
        .route("/v1/workspaces/search", get(workspaces::search))
        .route("/v1/workspaces/{id}", get(workspaces::get_node))
        .route("/v1/workspaces/{id}", delete(workspaces::delete_node))
        .route("/v1/workspaces/{id}/content", get(workspaces::get_content))
        .route("/v1/workspaces/{id}/content", patch(workspaces::patch_content))
        .route("/v1/workspaces/{id}/move", post(workspaces::move_node))
        .route("/v1/workspaces/{id}/share", post(workspaces::share_node))
        .route("/v1/workspaces/{id}/unshare", post(workspaces::unshare_node))
        // ── Workspace presign endpoints ─────────────────────────────────────
        .route("/v1/workspaces/{id}/presign-upload", post(workspaces::presign_upload))
        .route("/v1/workspaces/{id}/presign-download", get(workspaces::presign_download))
        // ── Workspace versioning ────────────────────────────────────────────
        .route("/v1/workspaces/nodes/{id}/versions", get(workspaces::list_versions))
        .route("/v1/workspaces/nodes/{id}/restore", post(workspaces::restore_version))
        // ── Tasks (background job polling + SSE) ────────────────────────────
        .route("/v1/tasks", get(tasks::list_tasks))
        .route("/v1/tasks/{id}", get(tasks::get_task))
        .route("/v1/tasks/{id}/sse", get(tasks::task_sse))
        // ── Threads ─────────────────────────────────────────────────────────
        .route("/v1/threads/{id}/messages", get(threads::get_messages))
        // ── Realtime ────────────────────────────────────────────────────────
        .route("/api/realtime/workspace", get(realtime::realtime_workspace))
        // ── Shell control ────────────────────────────────────────────────────
        .route("/v1/shells/{device_id}/control", get(shells::shell_control))
        // ── Billing ─────────────────────────────────────────────────────────
        .route("/v1/billing/plans", get(billing::list_plans))
        .route("/v1/billing/subscription", get(billing::get_subscription))
        .route("/v1/billing/subscriptions", post(billing::create_subscription))
        .route("/v1/billing/subscription", delete(billing::cancel_subscription))
        .route("/v1/billing/portal", post(billing::billing_portal))
        .route("/v1/billing/invoices", get(billing::list_invoices))
        .route("/v1/billing/usage", get(billing::get_usage))
}
