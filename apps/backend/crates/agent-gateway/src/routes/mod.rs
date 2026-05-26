use crate::mw::RouterQuotaLayer;
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    Router,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

mod admin_capabilities;
pub(crate) mod admin_devices;
mod admin_jobs;
mod admin_tenants;
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

const DEFAULT_JSON_BODY_LIMIT: usize = 256 * 1024;
const CHAT_BODY_LIMIT: usize = 2 * 1024 * 1024;
const WORKSPACE_CONTENT_BODY_LIMIT: usize = 8 * 1024 * 1024;
const WEBHOOK_BODY_LIMIT: usize = 256 * 1024;
const CAPABILITY_REGISTER_BODY_LIMIT: usize = 1024 * 1024;

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
        workspaces::rename_node,
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

// ── Static route table (source of truth for CI diff guard) ───────────────────

/// One entry per HTTP route. Used by `--dump-routes` and `make verify-routes-doc`.
pub struct RouteEntry {
    pub method: &'static str,
    pub path: &'static str,
    pub auth: &'static str,
    pub router: &'static str,
}

pub const ROUTE_TABLE: &[RouteEntry] = &[
    // Public
    RouteEntry {
        method: "GET",
        path: "/health",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "GET",
        path: "/healthz/embeddings",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "GET",
        path: "/login",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/auth/login",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/auth/legacy/login",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/billing/webhooks",
        auth: "hmac-sig",
        router: "public",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities/register",
        auth: "platform-token",
        router: "public",
    },
    RouteEntry {
        method: "GET",
        path: "/openapi.json",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "GET",
        path: "/docs",
        auth: "none",
        router: "public",
    },
    RouteEntry {
        method: "GET",
        path: "/metrics",
        auth: "none",
        router: "public",
    },
    // Internal (restrict by network in prod)
    RouteEntry {
        method: "POST",
        path: "/internal/rustfs/events",
        auth: "none",
        router: "internal",
    },
    // Protected (bearer/session/api-key + plan enforcement)
    RouteEntry {
        method: "POST",
        path: "/v1/chat/completions",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/agent/completions",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/capabilities",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/capabilities/search",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/mcp",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/files/upload-url",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/files/download-url",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/uploads/initiate",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/uploads/{upload_id}/parts/{n}/presign",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/uploads/{upload_id}/complete",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/uploads/{upload_id}/abort",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/audit",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/tree",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/search",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/{id}",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "DELETE",
        path: "/v1/workspaces/{id}",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/{id}/content",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "PATCH",
        path: "/v1/workspaces/{id}/content",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/{id}/move",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/{id}/rename",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/{id}/share",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/{id}/unshare",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/{id}/presign-upload",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/{id}/presign-download",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/workspaces/nodes/{id}/versions",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/workspaces/nodes/{id}/restore",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/tasks",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/tasks/{id}",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/tasks/{id}/sse",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/threads",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/threads/{id}/messages",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/api/realtime/workspace",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/shells/{device_id}/control",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/billing/plans",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/billing/subscription",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/billing/subscriptions",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "DELETE",
        path: "/v1/billing/subscription",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "POST",
        path: "/v1/billing/portal",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/billing/invoices",
        auth: "bearer",
        router: "protected",
    },
    RouteEntry {
        method: "GET",
        path: "/v1/billing/usage",
        auth: "bearer",
        router: "protected",
    },
    // Admin (super_admin JWT required)
    RouteEntry {
        method: "GET",
        path: "/admin/capabilities",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities/reload",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities/validate",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities/test",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/capabilities/{name}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/capabilities/{name}/manifest",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "PATCH",
        path: "/admin/capabilities/{name}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "PATCH",
        path: "/admin/capabilities/{name}/enabled",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "DELETE",
        path: "/admin/capabilities/{name}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/capabilities/{name}/reload",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/capabilities/namespaces",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/jobs",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/jobs/{name}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/jobs/{name}/run",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/tasks",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/devices",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/devices",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "DELETE",
        path: "/admin/devices/{id}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/billing/credits",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "POST",
        path: "/admin/billing/cancel/{tenant_id}",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "GET",
        path: "/admin/billing/dashboard",
        auth: "super-admin",
        router: "admin",
    },
    RouteEntry {
        method: "DELETE",
        path: "/admin/tenants/{id}",
        auth: "super-admin",
        router: "admin",
    },
];

/// Print ROUTE_TABLE as Markdown, grouped by router section.
pub fn dump_routes_markdown() -> String {
    let mut out = String::from("# ConusAI Gateway — Route Table\n\n");
    out.push_str("> Generated by `--dump-routes`. Do not edit manually.\n\n");
    for section in ["public", "internal", "protected", "admin"] {
        let rows: Vec<_> = ROUTE_TABLE.iter().filter(|r| r.router == section).collect();
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("## {section}\n\n"));
        out.push_str("| Method | Path | Auth |\n");
        out.push_str("|--------|------|------|\n");
        for r in rows {
            out.push_str(&format!("| `{}` | `{}` | {} |\n", r.method, r.path, r.auth));
        }
        out.push('\n');
    }
    out
}

/// Routes that require no auth (health probe, auth, OpenAPI).
/// Note: the old `GET /v1/files/{token}` UUID download shim is REMOVED.
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        .route("/healthz/embeddings", get(health::embeddings_ready))
        .route("/login", get(auth::login_page))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/legacy/login", post(auth::login))
        // Lago billing webhooks — signature verified inside handler.
        .route(
            "/v1/billing/webhooks",
            post(billing_webhook::handle_webhook).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
        )
        // Self-registration for external capability services.
        .route(
            "/admin/capabilities/register",
            post(admin_capabilities::register_capability)
                .layer(DefaultBodyLimit::max(CAPABILITY_REGISTER_BODY_LIMIT)),
        )
        // OpenAPI spec + Swagger UI
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .layer(DefaultBodyLimit::max(DEFAULT_JSON_BODY_LIMIT))
}

/// Super-admin only routes (JWT-protected with role=super_admin).
pub fn admin_router() -> Router<Arc<AppState>> {
    use crate::mw::admin::require_super_admin_jwt;
    use axum::middleware;
    Router::new()
        .route("/admin/capabilities", get(admin_capabilities::list))
        .route(
            "/admin/capabilities",
            post(admin_capabilities::create)
                .layer(DefaultBodyLimit::max(CAPABILITY_REGISTER_BODY_LIMIT)),
        )
        .route(
            "/admin/capabilities/reload",
            post(admin_capabilities::reload_all),
        )
        .route(
            "/admin/capabilities/validate",
            post(admin_capabilities::validate)
                .layer(DefaultBodyLimit::max(CAPABILITY_REGISTER_BODY_LIMIT)),
        )
        .route(
            "/admin/capabilities/test",
            post(admin_capabilities::test_invoke)
                .layer(DefaultBodyLimit::max(CAPABILITY_REGISTER_BODY_LIMIT)),
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
            axum::routing::patch(admin_capabilities::update)
                .layer(DefaultBodyLimit::max(CAPABILITY_REGISTER_BODY_LIMIT)),
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
        .route(
            "/admin/billing/cancel/{tenant_id}",
            post(billing::admin_cancel_subscription),
        )
        .route(
            "/admin/billing/dashboard",
            get(billing::admin_billing_dashboard),
        )
        // ── Tenant lifecycle ─────────────────────────────────────────────────
        .route("/admin/tenants/{id}", delete(admin_tenants::delete_tenant))
        .layer(middleware::from_fn(require_super_admin_jwt))
        .layer(DefaultBodyLimit::max(DEFAULT_JSON_BODY_LIMIT))
}

/// Internal routes — not exposed externally (mount behind a firewall or IP allowlist in prod).
pub fn internal_router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/internal/rustfs/events",
        post(internal::rustfs_events).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
    )
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
        .route(
            "/v1/chat/completions",
            post(chat::completions).layer(DefaultBodyLimit::max(CHAT_BODY_LIMIT)),
        )
        // Agent with tool calling + optional thread memory
        .route(
            "/v1/agent/completions",
            post(agent::agent_completions).layer(DefaultBodyLimit::max(CHAT_BODY_LIMIT)),
        )
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
        .route(
            "/v1/uploads/{upload_id}/parts/{n}/presign",
            post(uploads::presign_part),
        )
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
        .route(
            "/v1/workspaces/{id}/content",
            patch(workspaces::patch_content)
                .layer(DefaultBodyLimit::max(WORKSPACE_CONTENT_BODY_LIMIT)),
        )
        .route("/v1/workspaces/{id}/move", post(workspaces::move_node))
        .route("/v1/workspaces/{id}/rename", post(workspaces::rename_node))
        .route("/v1/workspaces/{id}/share", post(workspaces::share_node))
        .route(
            "/v1/workspaces/{id}/unshare",
            post(workspaces::unshare_node),
        )
        // ── Workspace presign endpoints ─────────────────────────────────────
        .route(
            "/v1/workspaces/{id}/presign-upload",
            post(workspaces::presign_upload),
        )
        .route(
            "/v1/workspaces/{id}/presign-download",
            get(workspaces::presign_download),
        )
        // ── Workspace versioning ────────────────────────────────────────────
        .route(
            "/v1/workspaces/nodes/{id}/versions",
            get(workspaces::list_versions),
        )
        .route(
            "/v1/workspaces/nodes/{id}/restore",
            post(workspaces::restore_version),
        )
        // ── Tasks (background job polling + SSE) ────────────────────────────
        .route("/v1/tasks", get(tasks::list_tasks))
        .route("/v1/tasks/{id}", get(tasks::get_task))
        .route("/v1/tasks/{id}/sse", get(tasks::task_sse))
        // ── Threads ─────────────────────────────────────────────────────────
        .route("/v1/threads", get(threads::list))
        .route("/v1/threads/{id}/messages", get(threads::get_messages))
        // ── Realtime ────────────────────────────────────────────────────────
        .route("/api/realtime/workspace", get(realtime::realtime_workspace))
        // ── Shell control ────────────────────────────────────────────────────
        .route("/v1/shells/{device_id}/control", get(shells::shell_control))
        // ── Billing ─────────────────────────────────────────────────────────
        .route("/v1/billing/plans", get(billing::list_plans))
        .route("/v1/billing/subscription", get(billing::get_subscription))
        .route(
            "/v1/billing/subscriptions",
            post(billing::create_subscription),
        )
        .route(
            "/v1/billing/subscription",
            delete(billing::cancel_subscription),
        )
        .route("/v1/billing/portal", post(billing::billing_portal))
        .route("/v1/billing/invoices", get(billing::list_invoices))
        .route("/v1/billing/usage", get(billing::get_usage))
        .layer(DefaultBodyLimit::max(DEFAULT_JSON_BODY_LIMIT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{PlanTier, TenantClaims, UserRole};
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
    };
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use std::sync::{Mutex, OnceLock};
    use tower::ServiceExt;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn admin_token(role: UserRole) -> String {
        encode(
            &Header::new(Algorithm::HS256),
            &TenantClaims {
                sub: "admin-user".into(),
                tenant_id: "tenant-admin".into(),
                plan: PlanTier::Enterprise,
                role,
                subscription_status: agent_core::SubscriptionStatus::Active,
                exp: 4_102_444_800,
            },
            &EncodingKey::from_secret(b"router-order-test-secret"),
        )
        .expect("jwt")
    }

    fn admin_app(state: Arc<AppState>) -> Router {
        Router::new()
            .merge(
                admin_router().layer(middleware::from_fn_with_state(
                    Arc::clone(&state),
                    crate::mw::tenant::extract_tenant,
                )),
            )
            .with_state(state)
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn admin_router_allows_super_admin_after_tenant_extraction() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("JWT_SECRET", "router-order-test-secret");
        }

        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));
        let app = admin_app(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/admin/jobs")
                    .header("authorization", format!("Bearer {}", admin_token(UserRole::SuperAdmin)))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn admin_router_rejects_non_super_admin_after_tenant_extraction() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("JWT_SECRET", "router-order-test-secret");
        }

        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));
        let app = admin_app(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/admin/jobs")
                    .header("authorization", format!("Bearer {}", admin_token(UserRole::User)))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn public_billing_webhook_rejects_oversized_payload() {
        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));
        let app = public_router().with_state(state);
        let oversized = vec![b'a'; WEBHOOK_BODY_LIMIT + 1];

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/billing/webhooks")
                    .header("content-type", "application/json")
                    .body(Body::from(oversized))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn admin_router_rejects_oversized_job_run_payload() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("JWT_SECRET", "router-order-test-secret");
        }

        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));
        let app = admin_app(state);
        let oversized = vec![b'a'; DEFAULT_JSON_BODY_LIMIT + 1];
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/jobs/example/run")
                    .header(
                        "authorization",
                        format!("Bearer {}", admin_token(UserRole::SuperAdmin)),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(oversized))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
