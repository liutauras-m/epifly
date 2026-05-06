use crate::state::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;
use utoipa::Modify;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

mod admin_capabilities;
mod admin_jobs;
pub mod agent;
mod audit;
pub mod auth;
mod capabilities;
pub mod chat;
mod files;
mod health;
mod mcp;
pub mod realtime;
mod search;
mod tasks;
mod threads;
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
        files::upload,
    ),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "chat", description = "OpenAI-compatible chat completions"),
        (name = "agent", description = "Thread-aware agent completions with tool calling"),
        (name = "capabilities", description = "Tool capability registry"),
        (name = "mcp", description = "MCP JSON-RPC 2.0 tool protocol"),
        (name = "workspaces", description = "Hierarchical workspace management"),
        (name = "audit", description = "Audit log"),
        (name = "files", description = "File storage"),
    ),
)]
pub struct ApiDoc;

/// Routes that require no auth (health probe, auth, OpenAPI).
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        // Auth: exchange credentials for JWT
        .route("/v1/auth/login", post(auth::login))
        // OpenAPI spec (machine-readable) + Swagger UI
        // SwaggerUi registers GET /openapi.json itself, so we don't add a
        // separate route to avoid an "Overlapping method route" panic.
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
        // Job management
        .route("/admin/jobs", get(admin_jobs::list_jobs))
        .route("/admin/jobs/{name}", get(admin_jobs::get_job))
        .route("/admin/jobs/{name}/run", post(admin_jobs::run_now))
        .route("/admin/tasks", get(admin_jobs::list_tasks))
        .layer(middleware::from_fn(require_super_admin_jwt))
}

/// Routes protected by the tenant middleware.
pub fn protected_router() -> Router<Arc<AppState>> {
    Router::new()
        // OpenAI-compatible chat
        .route("/v1/chat/completions", post(chat::completions))
        // Agent with tool calling + optional thread memory
        .route("/v1/agent/completions", post(agent::agent_completions))
        // Tool registry (path kept as /v1/capabilities for API compatibility)
        .route("/v1/capabilities", get(capabilities::list_capabilities))
        // Semantic capability search (Postgres pgvector ANN)
        .route("/v1/capabilities/search", get(search::search))
        // MCP JSON-RPC 2.0
        .route("/mcp", post(mcp::dispatch))
        // File storage (MinIO-backed)
        .route("/v1/files", post(files::upload))
        .route("/v1/files/{token}", get(files::download))
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
            patch(workspaces::patch_content),
        )
        .route("/v1/workspaces/{id}/move", post(workspaces::move_node))
        .route("/v1/workspaces/{id}/share", post(workspaces::share_node))
        .route(
            "/v1/workspaces/{id}/unshare",
            post(workspaces::unshare_node),
        )
        // ── Tasks (background job polling + SSE) ────────────────────────────
        .route("/v1/tasks", get(tasks::list_tasks))
        .route("/v1/tasks/{id}", get(tasks::get_task))
        .route("/v1/tasks/{id}/sse", get(tasks::task_sse))
        // ── Threads ─────────────────────────────────────────────────────────
        .route("/v1/threads/{id}/messages", get(threads::get_messages))
        // ── Realtime ────────────────────────────────────────────────────────
        .route("/api/realtime/workspace", get(realtime::realtime_workspace))
}
