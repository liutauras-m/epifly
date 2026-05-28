/// Workspace CRUD routes — folders, conversations, content, sharing, versioning, presign.
///
/// All routes require the tenant middleware (Extension<ResolvedTenant>).
/// Access model: every node is private to owner_id; sharing is explicit per node.
mod access;
mod content_indexing;
mod errors;
mod presign;
mod versioning;

pub use presign::{presign_download, presign_upload};
pub use versioning::{list_versions, restore_version};

use access::{cleanup_after_delete, maybe_provision_root_listing};
use content_indexing::enqueue_reindex;
use errors::{map_content_err, map_err};

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{ProjectionStatus, WorkspaceChangeEvent};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use common::error::HttpError;
use common::memory::workspace::{
    NodeKind, WorkspaceNode, WorkspaceNodeKind, effective_user_id, normalize_tags, validate_name,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

// ── Step 3.4 helper ──────────────────────────────────────────────────────────

/// Return `(primary_key, legacy_key)` for content store calls.
fn node_content_keys(node: &WorkspaceNode) -> (&str, Option<&str>) {
    match &node.object_key {
        Some(ok) => (ok.as_str(), Some(node.virtual_path.as_str())),
        None => (node.virtual_path.as_str(), None),
    }
}

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

#[derive(Deserialize)]
pub struct RenameBody {
    pub name: String,
}

#[derive(Serialize)]
pub struct ContentResponse {
    pub content: String,
}

fn apply_cursor(mut nodes: Vec<WorkspaceNode>, after: Option<Ulid>) -> Vec<WorkspaceNode> {
    if let Some(cursor) = after
        && let Some(pos) = nodes.iter().position(|n| n.id == cursor)
    {
        nodes = nodes.into_iter().skip(pos + 1).collect();
    }
    nodes
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
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
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
            // Write empty body to content store, then create workspace node.
            let node = state
                .workspace_store
                .create_conversation(&tenant.tenant_id, owner, body.parent_id, &body.name)
                .await
                .map_err(map_err)?;

            // Write empty .md to RustFS (best-effort; don't fail if RustFS is slow).
            // Dual-write via node_content_keys: new stable key primary, legacy best-effort.
            let (key, legacy) = node_content_keys(&node);
            let _ = state
                .workspace_content
                .write(&tenant.tenant_id, key, legacy, "")
                .await;

            state
                .realtime_service
                .publish_workspace_change(WorkspaceChangeEvent {
                    op: "workspace.created".into(),
                    tenant_id: tenant.tenant_id.to_string(),
                    node_id: node.id.to_string(),
                    kind: format!("{:?}", node.kind).to_lowercase(),
                })
                .await;

            return Ok(Json(node));
        }
        NodeKind::File => {
            return Err(HttpError::validation(
                "kind",
                "files are created via /v1/files",
            ));
        }
    }
    .map_err(map_err)?;

    state
        .realtime_service
        .publish_workspace_change(WorkspaceChangeEvent {
            op: "workspace.created".into(),
            tenant_id: tenant.tenant_id.to_string(),
            node_id: node.id.to_string(),
            kind: format!("{:?}", node.kind).to_lowercase(),
        })
        .await;

    Ok(Json(node))
}

/// GET /v1/workspaces/search?q=&limit= — search workspace nodes.
///
/// `mode=semantic` (or `mode=context`) uses embedding + ANN retrieval from
/// `content_embeddings`.  Default mode uses Postgres full-text search on node
/// names and virtual paths.
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

    if semantic {
        // Step 1: embed the query.
        // embed_query() already prepends the "query: " prefix required by multilingual-e5.
        let embedding = match state.embedding_service.embed_query(&query).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "semantic search: embed_query failed — falling back to name search");
                let nodes = state
                    .workspace_store
                    .search_nodes(&tenant.tenant_id, user, &query, limit)
                    .await
                    .map_err(map_err)?;
                return Ok(Json(apply_cursor(nodes, q.after)));
            }
        };

        // Step 2: ANN search — over-fetch (limit * 3) so that after dedup we still have
        // enough candidates.  top_n_content returns one row per chunk, so the same node
        // may appear multiple times.
        let ann_limit = (limit * 3).max(60);
        tracing::info!(
            query = %query,
            embedding_dims = embedding.len(),
            tenant_id = %tenant.tenant_id,
            ann_limit,
            "semantic search: querying Qdrant"
        );
        let hits = match state
            .vector_store
            .top_n_content(&embedding, ann_limit, &tenant.tenant_id)
            .await
        {
            Ok(h) => {
                tracing::info!(hits = h.len(), "semantic search: Qdrant returned hits");
                h
            }
            Err(e) => {
                tracing::warn!(error = %e, "semantic search: Qdrant query failed — falling back to name search");
                let nodes = state
                    .workspace_store
                    .search_nodes(&tenant.tenant_id, user, &query, limit)
                    .await
                    .map_err(map_err)?;
                return Ok(Json(apply_cursor(nodes, q.after)));
            }
        };

        // Step 3: deduplicate by node_id, preserving best-score order.
        let mut seen = std::collections::HashSet::new();
        let unique_ids: Vec<String> = hits
            .into_iter()
            .filter_map(|h| {
                if seen.insert(h.node_id.clone()) {
                    Some(h.node_id)
                } else {
                    None
                }
            })
            .take(limit * 2)
            .collect();

        // Step 4: hydrate nodes through the workspace store — this enforces access
        // control and returns full WorkspaceNode structs.  Skip hidden or inaccessible
        // nodes silently so the result set is clean.
        let mut nodes = Vec::with_capacity(unique_ids.len().min(limit));
        for id_str in unique_ids {
            if nodes.len() >= limit {
                break;
            }
            let node_id = match id_str.parse::<Ulid>() {
                Ok(id) => id,
                Err(_) => continue,
            };
            match state
                .workspace_store
                .get_accessible_node(&tenant.tenant_id, user, node_id)
                .await
            {
                Ok(node) if node.hidden_at.is_none() => nodes.push(node),
                _ => {}
            }
        }

        return Ok(Json(apply_cursor(nodes, q.after)));
    }

    let nodes = state
        .workspace_store
        .search_nodes(&tenant.tenant_id, user, &query, limit)
        .await
        .map_err(map_err)?;

    Ok(Json(apply_cursor(nodes, q.after)))
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

    // § 1.6 Runtime safety net: first-time root listing for unseeded tenants.
    // If this is a root-level query, the list is empty, and the tenant has not
    // been seeded yet, run idempotent provisioning (single-flight per tenant).
    if q.parent_id.is_none()
        && nodes.is_empty()
        && let Some(ref onboarding) = state.onboarding
    {
        let did_provision = maybe_provision_root_listing(
            &state,
            tenant.tenant_id.as_ref(),
            || async {
                state
                    .workspace_store
                    .is_tenant_seeded(&tenant.tenant_id)
                    .await
                    .unwrap_or(true)
            },
            || async {
                use agent_core::store::onboarding::{OnboardingOptions, TenantKind};
                #[cfg(debug_assertions)]
                let owner: &str = tenant.user_id.as_deref().unwrap_or("__dev__");
                #[cfg(not(debug_assertions))]
                let owner: &str = tenant
                    .user_id
                    .as_deref()
                    .ok_or_else(|| HttpError::agent("tenant has no resolved user"))?;
                let opts = OnboardingOptions {
                    kind: TenantKind::Normal,
                    root_name: None,
                };
                if let Err(e) = onboarding.provision(&tenant.tenant_id, owner, opts).await {
                    tracing::warn!(
                        error = %e,
                        tenant_id = %tenant.tenant_id,
                        "tree safety-net: onboarding provision failed"
                    );
                }
                Ok(())
            },
        )
        .await?;

        if did_provision {
            // Re-fetch after provisioning so the root folder appears immediately.
            let fresh = state
                .workspace_store
                .list_accessible_children(&tenant.tenant_id, user, None)
                .await
                .map_err(map_err)?;
            return Ok(Json(apply_cursor(fresh, q.after)));
        }
    }

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

    let (key, legacy) = node_content_keys(&node);
    let content = state
        .workspace_content
        .read(&tenant.tenant_id, key, legacy)
        .await
        .map_err(map_content_err)?;

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
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }

    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let (key, legacy) = node_content_keys(&node);
    state
        .workspace_content
        .write(&tenant.tenant_id, key, legacy, &body.content)
        .await
        .map_err(map_content_err)?;

    enqueue_reindex(
        &state,
        tenant.tenant_id.to_string(),
        id,
        node.last_modified.timestamp_millis(),
    )
    .await;

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
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }
    let user = effective_user_id(tenant.user_id.as_deref());

    // Capture old virtual_path before the move so we can copy the object key.
    let old_virtual_path = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)
        .map(|n| n.virtual_path)
        .ok();

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

    // Copy object to new legacy key when virtual_path changed.
    // Step 3.4: skip for nodes with a stable object_key — content is at the
    // node-id-keyed path and does not move with the virtual_path.
    if node.object_key.is_none()
        && let Some(old_path) = old_virtual_path
        && node.virtual_path != old_path
    {
        match state
            .workspace_content
            .read(&tenant.tenant_id, &old_path, None)
            .await
        {
            Ok(content) if !content.is_empty() => {
                if let Err(e) = state
                    .workspace_content
                    .write(&tenant.tenant_id, &node.virtual_path, None, &content)
                    .await
                {
                    tracing::warn!(error = %e, "move_node: failed to copy object to new key");
                } else {
                    let _ = state
                        .workspace_content
                        .delete(&tenant.tenant_id, &old_path, None)
                        .await;
                }
            }
            _ => {}
        }
    }

    Ok(Json(node))
}

/// POST /v1/workspaces/:id/rename — rename a workspace node.
///
/// Protected root folders can only be renamed by users with `tenant:admin` role.
#[utoipa::path(
    post,
    path = "/v1/workspaces/{id}/rename",
    params(("id" = String, Path, description = "Node ULID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Updated node", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant, body))]
pub async fn rename_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<RenameBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    use agent_core::context::tenant::UserRole;
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }
    let user = effective_user_id(tenant.user_id.as_deref());

    // Check if node is a protected root — only admins may rename it.
    if let Ok(node) = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        && node.is_protected_root
        && tenant.role == UserRole::User
    {
        return Err(HttpError::forbidden(
            "only admins may rename the workspace root folder",
        ));
    }

    let updated = state
        .workspace_store
        .rename_node(&tenant.tenant_id, user, id, body.name)
        .await
        .map_err(map_err)?;

    Ok(Json(updated))
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
pub async fn share_node(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<ShareBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
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
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
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
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }
    let user = effective_user_id(tenant.user_id.as_deref());

    // Fetch the node to check its semantic kind before planning deletion.
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // Step 5.6: Thread-kind nodes use delete-as-pause, not hard delete.
    if node.semantic_kind == WorkspaceNodeKind::Thread {
        // Pause the projection.
        if let Some(thread_id) = node.source_id.as_deref() {
            let _ = state
                .thread_projection_store
                .set_status(
                    &tenant.tenant_id,
                    thread_id,
                    agent_core::ProjectionStatus::Paused,
                )
                .await;
        }
        // Soft-hide the node.
        state
            .workspace_store
            .hide_node(&tenant.tenant_id, id)
            .await
            .map_err(map_err)?;

        state
            .realtime_service
            .publish_workspace_change(WorkspaceChangeEvent {
                op: "workspace.hidden".into(),
                tenant_id: tenant.tenant_id.to_string(),
                node_id: id.to_string(),
                kind: "thread".into(),
            })
            .await;
        return Ok(StatusCode::NO_CONTENT);
    }

    // Standard delete for non-Thread nodes.
    let plan = state
        .workspace_store
        .plan_delete(&tenant.tenant_id, id)
        .await
        .map_err(map_err)?;

    state
        .workspace_store
        .delete_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // Best-effort cleanup — never fail the API response on cleanup errors.
    cleanup_after_delete(&state, &tenant.tenant_id, &plan).await;

    state
        .realtime_service
        .publish_workspace_change(WorkspaceChangeEvent {
            op: "workspace.deleted".into(),
            tenant_id: tenant.tenant_id.to_string(),
            node_id: id.to_string(),
            kind: "node".into(),
        })
        .await;

    Ok(StatusCode::NO_CONTENT)
}

// ── Tags ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PutTagsBody {
    pub tags: Vec<String>,
}

/// `PUT /v1/workspaces/{id}/tags` — replace the tag set on a node.
#[utoipa::path(
    put,
    path = "/v1/workspaces/{id}/tags",
    params(("id" = String, Path, description = "Node ULID")),
    responses((status = 200), (status = 404,), (status = 400,)),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn put_tags(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    axum::Json(body): axum::Json<PutTagsBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    let tags = normalize_tags(body.tags).map_err(|e| HttpError::bad_request(e.to_string()))?;

    let user = effective_user_id(tenant.user_id.as_deref());
    let mut node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    node.tags = tags;
    state
        .workspace_store
        .upsert_node(node.clone())
        .await
        .map_err(map_err)?;

    Ok(Json(node))
}

// ── Filtered search ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FilterSearchQuery {
    pub tag: Option<String>,
    pub kind: Option<String>,
    pub since: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
}

/// `GET /v1/workspaces/filter` — filter nodes by tag and/or semantic_kind.
///
/// Used by the UI's "Type: Files / Threads" and "Time: Today / This week" filters.
#[utoipa::path(
    get,
    path = "/v1/workspaces/filter",
    params(
        ("tag" = Option<String>, Query, description = "Filter by tag (exact match)"),
        ("kind" = Option<String>, Query, description = "folder | file | thread"),
        ("since" = Option<String>, Query, description = "ISO-8601 datetime"),
        ("q" = Option<String>, Query, description = "Text search prefix"),
        ("limit" = Option<usize>, Query, description = "Max results (default 50)"),
    ),
    responses((status = 200)),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn filter_nodes(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    axum::extract::Query(params): axum::extract::Query<FilterSearchQuery>,
) -> Result<Json<Vec<WorkspaceNode>>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let limit = params.limit.unwrap_or(50).min(200);

    // Start from a text search if `q` is given; otherwise scan all tenant nodes so
    // tag/kind filters can reach nested nodes (not just root-level children).
    let q = params.q.as_deref().unwrap_or("");
    let mut nodes = state
        .workspace_store
        .search_nodes(&tenant.tenant_id, user, q, limit * 4)
        .await
        .map_err(map_err)?;

    // Filter: tag
    if let Some(ref tag) = params.tag {
        let norm = tag.trim().to_lowercase();
        nodes.retain(|n| n.tags.iter().any(|t| t == &norm));
    }

    // Filter: semantic_kind
    if let Some(ref kind_str) = params.kind {
        let filter_kind: Option<WorkspaceNodeKind> = match kind_str.as_str() {
            "folder" => Some(WorkspaceNodeKind::Folder),
            "file" => Some(WorkspaceNodeKind::File),
            "thread" => Some(WorkspaceNodeKind::Thread),
            _ => None,
        };
        if let Some(fk) = filter_kind {
            nodes.retain(|n| n.semantic_kind == fk);
        }
    }

    // Filter: since (ISO-8601)
    if let Some(ref since_str) = params.since
        && let Ok(since) = since_str.parse::<chrono::DateTime<chrono::Utc>>()
    {
        nodes.retain(|n| n.last_modified >= since);
    }

    // Filter: hidden (Thread-kind nodes in hidden state are excluded by default)
    nodes.retain(|n| n.hidden_at.is_none());

    nodes.truncate(limit);
    Ok(Json(nodes))
}

// ── Thread projection restore ─────────────────────────────────────────────────

/// `POST /v1/threads/{thread_id}/projection/restore` — un-hide a paused thread projection.
///
/// Clears `hidden_at`, sets projection status to `Active`, enqueues a fresh projection job.
#[utoipa::path(
    post,
    path = "/v1/threads/{thread_id}/projection/restore",
    params(("thread_id" = String, Path, description = "Thread ID")),
    responses((status = 204,), (status = 404,)),
    security(("bearer_auth" = [])),
    tag = "workspaces",
)]
#[instrument(skip(state, tenant))]
pub async fn restore_thread_projection(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
) -> Result<StatusCode, HttpError> {
    // Lookup the projection record to find the node_id.
    let proj = state
        .thread_projection_store
        .get(&tenant.tenant_id, &thread_id)
        .await
        .map_err(|e| HttpError::internal(e.to_string(), None))?
        .ok_or_else(|| HttpError::not_found("thread projection not found"))?;

    // Un-hide the node.
    state
        .workspace_store
        .unhide_node(&tenant.tenant_id, proj.node_id)
        .await
        .map_err(map_err)?;

    // Activate the projection.
    state
        .thread_projection_store
        .set_status(&tenant.tenant_id, &thread_id, ProjectionStatus::Active)
        .await
        .map_err(|e| HttpError::internal(e.to_string(), None))?;

    // Enqueue a fresh projection run.
    {
        use jobs::jobs::{ProjectionReason, ThreadProjectionInput};
        let input = serde_json::to_value(ThreadProjectionInput {
            tenant_id: tenant.tenant_id.to_string(),
            thread_id: thread_id.clone(),
            reason: ProjectionReason::ManualReproject,
            folder_path: Some(proj.folder_path.clone()),
        })
        .expect("ThreadProjectionInput serializable");

        let _ = state
            .job_executor
            .enqueue(jobs::jobs::ThreadProjectionJob::NAME, input)
            .await;
    }

    state
        .realtime_service
        .publish_workspace_change(WorkspaceChangeEvent {
            op: "workspace.restored".into(),
            tenant_id: tenant.tenant_id.to_string(),
            node_id: proj.node_id.to_string(),
            kind: "thread".into(),
        })
        .await;

    Ok(StatusCode::NO_CONTENT)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use chrono::Utc;
    use common::memory::workspace::WorkspaceNodeKind;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    fn test_state() -> AppState {
        AppState::with_in_memory_stores().expect("in-memory app state")
    }

    fn workspace_node(kind: NodeKind, virtual_path: &str) -> WorkspaceNode {
        use common::memory::workspace::WorkspaceNodeKind;
        let id = Ulid::new();
        WorkspaceNode {
            object_key: if kind == NodeKind::Folder {
                None
            } else {
                Some(format!("nodes/{id}/content"))
            },
            semantic_kind: if kind == NodeKind::Folder {
                WorkspaceNodeKind::Folder
            } else {
                WorkspaceNodeKind::File
            },
            id,
            tenant_id: "tenant-a".into(),
            owner_id: "user-a".into(),
            parent_id: None,
            kind,
            name: virtual_path
                .rsplit('/')
                .next()
                .unwrap_or(virtual_path)
                .into(),
            virtual_path: virtual_path.into(),
            last_modified: Utc::now(),
            shared_with: vec![],
            metadata: serde_json::json!({}),
            is_protected_root: false,
            source_type: None,
            source_id: None,
            hidden_at: None,
            tags: vec![],
        }
    }

    #[test]
    fn presign_file_node_uses_node_path_and_ignores_legacy_body_path() {
        use presign::resolve_presign_path;
        let node = workspace_node(NodeKind::File, "node/foo.txt");
        let resolved = resolve_presign_path(&node, Some("node/foo.txt/attachment.bin"))
            .expect("file node path should resolve");
        assert_eq!(resolved.as_str(), "node/foo.txt");
    }

    #[test]
    fn presign_folder_node_accepts_strict_child() {
        use presign::resolve_presign_path;
        let node = workspace_node(NodeKind::Folder, "node/foo");
        let resolved = resolve_presign_path(&node, Some("node/foo/attachment.bin"))
            .expect("child path should resolve");
        assert_eq!(resolved.as_str(), "node/foo/attachment.bin");
    }

    #[test]
    fn presign_folder_node_rejects_same_path() {
        use presign::{PresignPathError, resolve_presign_path};
        let node = workspace_node(NodeKind::Folder, "node/foo");
        let err = resolve_presign_path(&node, Some("node/foo")).unwrap_err();
        assert!(matches!(err, PresignPathError::OutsideNode));
    }

    #[test]
    fn presign_folder_node_rejects_sibling_prefix_attack() {
        use presign::{PresignPathError, resolve_presign_path};
        let node = workspace_node(NodeKind::Folder, "node/foo");
        let err = resolve_presign_path(&node, Some("node/foobar/secret.txt")).unwrap_err();
        assert!(matches!(err, PresignPathError::OutsideNode));
    }

    #[test]
    fn presign_folder_node_rejects_percent_encoded_traversal() {
        use presign::{PresignPathError, resolve_presign_path};
        let node = workspace_node(NodeKind::Folder, "node/foo");
        let err = resolve_presign_path(&node, Some("node/foo%2f..%2fsecret.txt")).unwrap_err();
        assert!(matches!(err, PresignPathError::InvalidRequestedPath(_)));
    }

    // ── Step 5.8: filter_nodes acceptance test ────────────────────────────────

    /// `?kind=thread&tag=invoices` must return ONLY Thread-kind nodes tagged "invoices".
    /// File-kind nodes and Thread nodes without the tag must both be excluded.
    #[tokio::test]
    async fn filter_nodes_returns_only_thread_kind_with_matching_tag() {
        let state = Arc::new(test_state());

        // Thread-kind node tagged "invoices" — MUST be returned.
        let mut match_node = workspace_node(NodeKind::File, "Conversations/thread1.md");
        match_node.tenant_id = "acme".into();
        match_node.owner_id = "__system__".into();
        match_node.semantic_kind = WorkspaceNodeKind::Thread;
        match_node.tags = vec!["invoices".into()];
        state
            .workspace_store
            .upsert_node(match_node.clone())
            .await
            .unwrap();

        // File-kind node also tagged "invoices" — must NOT be returned (wrong kind).
        let mut file_node = workspace_node(NodeKind::File, "Conversations/file1.md");
        file_node.tenant_id = "acme".into();
        file_node.owner_id = "__system__".into();
        file_node.tags = vec!["invoices".into()];
        state.workspace_store.upsert_node(file_node).await.unwrap();

        // Thread-kind node tagged "other" — must NOT be returned (wrong tag).
        let mut other_tag = workspace_node(NodeKind::File, "Conversations/thread2.md");
        other_tag.tenant_id = "acme".into();
        other_tag.owner_id = "__system__".into();
        other_tag.semantic_kind = WorkspaceNodeKind::Thread;
        other_tag.tags = vec!["other".into()];
        state.workspace_store.upsert_node(other_tag).await.unwrap();

        // Apply the same filtering logic as the filter_nodes handler.
        let mut nodes = state
            .workspace_store
            .list_accessible_children("acme", "__system__", None)
            .await
            .expect("list children");

        // kind=thread
        nodes.retain(|n| n.semantic_kind == WorkspaceNodeKind::Thread);
        // tag=invoices
        nodes.retain(|n| n.tags.iter().any(|t| t == "invoices"));
        // hidden_at filter
        nodes.retain(|n| n.hidden_at.is_none());

        assert_eq!(
            nodes.len(),
            1,
            "exactly one node should match kind=thread&tag=invoices"
        );
        assert_eq!(nodes[0].id, match_node.id);
    }

    // ── Provisioning tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn provision_called_once_when_unseeded() {
        let state = test_state();
        let seeded = Arc::new(Mutex::new(VecDeque::from([false, false])));
        let calls = Arc::new(AtomicU32::new(0));

        let did_provision = maybe_provision_root_listing(
            &state,
            "tenant-a",
            {
                let seeded = Arc::clone(&seeded);
                move || {
                    let seeded = Arc::clone(&seeded);
                    async move { seeded.lock().unwrap().pop_front().unwrap_or(true) }
                }
            },
            {
                let calls = Arc::clone(&calls);
                move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("helper should not fail");

        assert!(did_provision);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn provision_skipped_when_already_seeded() {
        let state = test_state();
        let seeded = Arc::new(Mutex::new(VecDeque::from([true])));
        let calls = Arc::new(AtomicU32::new(0));

        let did_provision = maybe_provision_root_listing(
            &state,
            "tenant-a",
            {
                let seeded = Arc::clone(&seeded);
                move || {
                    let seeded = Arc::clone(&seeded);
                    async move { seeded.lock().unwrap().pop_front().unwrap_or(true) }
                }
            },
            {
                let calls = Arc::clone(&calls);
                move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("helper should not fail");

        assert!(!did_provision);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provision_skipped_when_concurrent_seed_wins_lock() {
        let state = test_state();
        let seeded = Arc::new(Mutex::new(VecDeque::from([false, true])));
        let calls = Arc::new(AtomicU32::new(0));

        let did_provision = maybe_provision_root_listing(
            &state,
            "tenant-a",
            {
                let seeded = Arc::clone(&seeded);
                move || {
                    let seeded = Arc::clone(&seeded);
                    async move { seeded.lock().unwrap().pop_front().unwrap_or(true) }
                }
            },
            {
                let calls = Arc::clone(&calls);
                move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("helper should not fail");

        assert!(!did_provision);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
