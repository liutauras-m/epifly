/// Workspace CRUD routes — folders, conversations, content, sharing.
///
/// All routes require the tenant middleware (Extension<ResolvedTenant>).
/// Access model: every node is private to owner_id; sharing is explicit per node.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use common::error::HttpError;
use common::memory::workspace::{NodeKind, WorkspaceNode, effective_user_id, validate_name};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

// ── Request / response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateBody {
    pub kind: NodeKind,
    pub parent_id: Option<Ulid>,
    pub name: String,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TreeQuery {
    pub parent_id: Option<Ulid>,
    pub after: Option<Ulid>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
    pub mode: Option<String>,
    pub after: Option<Ulid>,
}

fn apply_cursor(mut nodes: Vec<WorkspaceNode>, after: Option<Ulid>) -> Vec<WorkspaceNode> {
    if let Some(cursor) = after
        && let Some(pos) = nodes.iter().position(|n| n.id == cursor)
    {
        nodes = nodes.into_iter().skip(pos + 1).collect();
    }
    nodes
}

fn semantic_terms(query: &str) -> Vec<String> {
    fn stem(t: &str) -> String {
        let mut s = t.to_lowercase();
        if s.ends_with("ing") && s.len() > 5 {
            s.truncate(s.len() - 3);
        } else if s.ends_with("ed") && s.len() > 4 {
            s.truncate(s.len() - 2);
        } else if s.ends_with('s') && s.len() > 3 {
            s.truncate(s.len() - 1);
        }
        s
    }

    fn synonym_set(token: &str) -> &'static [&'static str] {
        match token {
            "invoice" | "bill" | "receipt" => &["invoice", "bill", "receipt", "payment"],
            "meeting" | "minutes" | "notes" => {
                &["meeting", "minutes", "notes", "summary", "standup"]
            }
            "plan" | "roadmap" | "todo" => &["plan", "roadmap", "todo", "milestone", "task"],
            "bug" | "issue" | "error" => &["bug", "issue", "error", "fix", "defect"],
            "design" | "ux" | "ui" => &["design", "ux", "ui", "wireframe", "prototype"],
            "contract" | "agreement" => &["contract", "agreement", "terms", "clause"],
            "spec" | "requirement" => &["spec", "requirement", "acceptance", "criteria"],
            _ => &[],
        }
    }

    let mut out: Vec<String> = Vec::new();
    for raw in query.split_whitespace() {
        let token = stem(raw);
        if token.is_empty() {
            continue;
        }
        if !out.contains(&token) {
            out.push(token.clone());
        }
        for s in synonym_set(&token) {
            let ss = (*s).to_string();
            if !out.contains(&ss) {
                out.push(ss);
            }
        }
    }
    out
}

#[derive(Deserialize)]
pub struct ContentBody {
    pub content: String,
}

#[derive(Deserialize)]
pub struct MoveBody {
    pub new_parent_id: Option<Ulid>,
    pub new_parent_path: Option<String>,
}

#[derive(Deserialize)]
pub struct ShareBody {
    pub user_id: String,
}

#[derive(Serialize)]
pub struct ContentResponse {
    pub content: String,
}

// ── Error helper ─────────────────────────────────────────────────────────────

fn map_err(e: anyhow::Error) -> HttpError {
    let msg = e.to_string();
    if msg.contains("validation error") {
        HttpError::validation("body", msg)
    } else if msg.contains("not found") {
        HttpError::not_found(msg)
    } else {
        HttpError::internal(msg, None)
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /v1/workspaces — create a folder or conversation.
#[utoipa::path(
    post,
    path = "/v1/workspaces",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Created workspace node", body = serde_json::Value),
        (status = 400, description = "Validation error"),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Json(body): Json<CreateBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }

    // Validate name eagerly so we return 400, not 500
    validate_name(&body.name, body.kind)
        .map_err(|e| HttpError::validation("name", e.to_string()))?;

    let owner = effective_user_id(tenant.user_id.as_deref());

    let node = match body.kind {
        NodeKind::Folder => {
            state
                .workspace_store
                .create_folder(&tenant.tenant_id, owner, body.parent_id, &body.name)
                .await
        }
        NodeKind::Conversation => {
            // MinIO put first, then Qdrant upsert
            let node = state
                .workspace_store
                .create_conversation(&tenant.tenant_id, owner, body.parent_id, &body.name)
                .await
                .map_err(map_err)?;

            // Write empty .md to MinIO (best-effort; don't fail if MinIO is slow)
            let _ = state
                .workspace_content
                .write(&tenant.tenant_id, &node.virtual_path, "")
                .await;

            return Ok(Json(node));
        }
        NodeKind::File => {
            return Err(HttpError::validation("kind", "files are created via /v1/files"));
        }
    }
    .map_err(map_err)?;

    Ok(Json(node))
}

/// GET /v1/workspaces/search?q=&limit= — full-text search over workspace node names.
///
/// Uses Qdrant text_match on the `name` field (word-tokenised, lowercase).
/// Falls back to a local substring scan when the index isn't ready yet.
/// Returns a flat list of matching nodes (folders + conversations) the user can access.
#[utoipa::path(
    get,
    path = "/v1/workspaces/search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Matching workspace nodes", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<WorkspaceNode>>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let limit = q.limit.unwrap_or(40).min(200);
    let query = q.q.trim().to_string();
    let semantic = q
        .mode
        .as_deref()
        .map(|m| m.eq_ignore_ascii_case("context") || m.eq_ignore_ascii_case("semantic"))
        .unwrap_or(false);

    if query.is_empty() {
        return Ok(Json(vec![]));
    }

    if !semantic {
        let nodes = state
            .workspace_store
            .search_nodes(&tenant.tenant_id, user, &query, limit)
            .await
            .map_err(map_err)?;

        return Ok(Json(apply_cursor(nodes, q.after)));
    }

    let mut merged: Vec<WorkspaceNode> = Vec::new();
    let terms = semantic_terms(&query);
    for term in terms {
        let batch = state
            .workspace_store
            .search_nodes(&tenant.tenant_id, user, &term, limit)
            .await
            .map_err(map_err)?;
        for node in batch {
            if merged.iter().all(|n| n.id != node.id) {
                merged.push(node);
                if merged.len() >= limit {
                    return Ok(Json(apply_cursor(merged, q.after)));
                }
            }
        }
    }

    Ok(Json(apply_cursor(merged, q.after)))
}

/// GET /v1/workspaces/tree?parent_id= — list immediate children.
#[utoipa::path(
    get,
    path = "/v1/workspaces/tree",
    params(TreeQuery),
    responses(
        (status = 200, description = "Child workspace nodes", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn tree(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<TreeQuery>,
) -> Result<Json<Vec<WorkspaceNode>>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let nodes = state
        .workspace_store
        .list_accessible_children(&tenant.tenant_id, user, q.parent_id)
        .await
        .map_err(map_err)?;
    Ok(Json(apply_cursor(nodes, q.after)))
}

/// GET /v1/workspaces/:id — get a single node.
#[utoipa::path(
    get,
    path = "/v1/workspaces/{id}",
    params(("id" = String, Path, description = "Node ULID")),
    responses(
        (status = 200, description = "Workspace node", body = serde_json::Value),
        (status = 404, description = "Node not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn get_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;
    Ok(Json(node))
}

/// GET /v1/workspaces/:id/content — read markdown body.
#[utoipa::path(
    get,
    path = "/v1/workspaces/{id}/content",
    params(("id" = String, Path, description = "Node ULID")),
    responses(
        (status = 200, description = "Node content", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn get_content(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<Json<ContentResponse>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let content = state
        .workspace_content
        .read(&tenant.tenant_id, &node.virtual_path)
        .await
        .map_err(map_err)?;

    Ok(Json(ContentResponse { content }))
}

/// PATCH /v1/workspaces/:id/content — save markdown body.
#[utoipa::path(
    patch,
    path = "/v1/workspaces/{id}/content",
    params(("id" = String, Path, description = "Node ULID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Updated node", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn patch_content(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ContentBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }

    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // MinIO write first, then update Qdrant with content_text for search indexing.
    state
        .workspace_content
        .write(&tenant.tenant_id, &node.virtual_path, &body.content)
        .await
        .map_err(map_err)?;

    // index_content updates content_text in Qdrant payload AND bumps last_modified,
    // so we skip the separate bump_last_modified call.
    let _ = state
        .workspace_store
        .index_content(&tenant.tenant_id, id, &body.content)
        .await;

    // Return fresh node
    let updated = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;
    Ok(Json(updated))
}

/// POST /v1/workspaces/:id/move — reparent a node.
#[utoipa::path(
    post,
    path = "/v1/workspaces/{id}/move",
    params(("id" = String, Path, description = "Node ULID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Moved node", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn move_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<MoveBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .move_node(
            &tenant.tenant_id,
            user,
            id,
            body.new_parent_id,
            body.new_parent_path.as_deref(),
        )
        .await
        .map_err(map_err)?;
    Ok(Json(node))
}

/// POST /v1/workspaces/:id/share — add a user to shared_with.
#[utoipa::path(
    post,
    path = "/v1/workspaces/{id}/share",
    params(("id" = String, Path, description = "Node ULID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Updated node", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn share_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ShareBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }
    let owner = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .share_node(&tenant.tenant_id, owner, id, &body.user_id)
        .await
        .map_err(map_err)?;
    Ok(Json(node))
}

/// POST /v1/workspaces/:id/unshare — remove a user from shared_with.
#[utoipa::path(
    post,
    path = "/v1/workspaces/{id}/unshare",
    params(("id" = String, Path, description = "Node ULID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Updated node", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn unshare_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ShareBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }
    let owner = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .unshare_node(&tenant.tenant_id, owner, id, &body.user_id)
        .await
        .map_err(map_err)?;
    Ok(Json(node))
}

/// DELETE /v1/workspaces/:id — recursive delete (folders + content).
#[utoipa::path(
    delete,
    path = "/v1/workspaces/{id}",
    params(("id" = String, Path, description = "Node ULID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Node not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<StatusCode, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }
    let user = effective_user_id(tenant.user_id.as_deref());

    // Get the node first so we can clean up MinIO content
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // Best-effort MinIO cleanup for conversations
    if node.kind == NodeKind::Conversation {
        let _ = state
            .workspace_content
            .delete(&tenant.tenant_id, &node.virtual_path)
            .await;
    }

    state
        .workspace_store
        .delete_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    Ok(StatusCode::NO_CONTENT)
}
