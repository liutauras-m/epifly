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
use common::memory::workspace::{NodeKind, WorkspaceNode, effective_user_id, validate_name};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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

#[derive(Debug, Deserialize)]
pub struct TreeQuery {
    pub parent_id: Option<Ulid>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
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

fn map_err(e: anyhow::Error) -> (StatusCode, Json<Value>) {
    // Check if the anyhow error wraps a ConusAiError
    let msg = e.to_string();
    let code = if msg.contains("validation error") {
        StatusCode::BAD_REQUEST
    } else if msg.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (code, Json(json!({"error": msg})))
}

fn rate_limit_err() -> (StatusCode, Json<Value>) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"error": "rate limit exceeded"})),
    )
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /v1/workspaces — create a folder or conversation.
#[instrument(skip(state, tenant, body))]
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Json(body): Json<CreateBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
    }

    // Validate name eagerly so we return 400, not 500
    validate_name(&body.name, body.kind).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
    })?;

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
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "files are created via /v1/files"})),
            ));
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
#[instrument(skip(state, tenant))]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<WorkspaceNode>>, (StatusCode, Json<Value>)> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let limit = q.limit.unwrap_or(40).min(200);
    let query = q.q.trim().to_string();

    if query.is_empty() {
        return Ok(Json(vec![]));
    }

    let nodes = state
        .workspace_store
        .search_nodes(&tenant.tenant_id, user, &query, limit)
        .await
        .map_err(map_err)?;

    Ok(Json(nodes))
}

/// GET /v1/workspaces/tree?parent_id= — list immediate children.
#[instrument(skip(state, tenant))]
pub async fn tree(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<TreeQuery>,
) -> Result<Json<Vec<WorkspaceNode>>, (StatusCode, Json<Value>)> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let nodes = state
        .workspace_store
        .list_accessible_children(&tenant.tenant_id, user, q.parent_id)
        .await
        .map_err(map_err)?;
    Ok(Json(nodes))
}

/// GET /v1/workspaces/:id — get a single node.
#[instrument(skip(state, tenant))]
pub async fn get_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;
    Ok(Json(node))
}

/// GET /v1/workspaces/:id/content — read markdown body.
#[instrument(skip(state, tenant))]
pub async fn get_content(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<Json<ContentResponse>, (StatusCode, Json<Value>)> {
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
#[instrument(skip(state, tenant, body))]
pub async fn patch_content(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ContentBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
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
#[instrument(skip(state, tenant, body))]
pub async fn move_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<MoveBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
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
#[instrument(skip(state, tenant, body))]
pub async fn share_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ShareBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
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
#[instrument(skip(state, tenant, body))]
pub async fn unshare_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ShareBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
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
#[instrument(skip(state, tenant))]
pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())
    {
        return Err(rate_limit_err());
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
