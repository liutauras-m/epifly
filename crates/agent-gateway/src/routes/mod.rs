use crate::state::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;

pub mod agent;
mod audit;
mod capabilities;
pub mod chat;
mod files;
mod health;
mod mcp;
mod search;
mod threads;
mod workspaces;

/// Routes that require no auth (health probe, token-based file download).
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        // Token-based download: the token itself is the capability proof
        .route("/v1/files/{token}", get(files::download))
}

/// Routes protected by the tenant middleware.
pub fn protected_router() -> Router<Arc<AppState>> {
    Router::new()
        // OpenAI-compatible chat
        .route("/v1/chat/completions", post(chat::completions))
        // Agent with tool calling + optional thread memory
        .route("/v1/agent/completions", post(agent::agent_completions))
        // Capability registry
        .route("/v1/capabilities", get(capabilities::list_capabilities))
        // Semantic capability search (Qdrant-backed)
        .route("/v1/capabilities/search", get(search::search))
        // MCP JSON-RPC 2.0
        .route("/mcp", post(mcp::dispatch))
        // File storage (MinIO-backed)
        .route("/v1/files", post(files::upload))
        // ── Thread / persistent memory ─────────────────────────────────────
        .route("/v1/threads", post(threads::create_thread))
        .route("/v1/threads", get(threads::list_threads))
        .route("/v1/threads/{thread_id}", get(threads::get_thread))
        .route(
            "/v1/threads/{thread_id}/messages",
            get(threads::get_messages),
        )
        .route(
            "/v1/threads/{thread_id}/messages",
            post(threads::append_message),
        )
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
}
