/// Workspace CRUD routes — folders, conversations, content, sharing, versioning, presign.
///
/// All routes require the tenant middleware (Extension<ResolvedTenant>).
/// Access model: every node is private to owner_id; sharing is explicit per node.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{VirtualPath, WorkspaceChangeEvent};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use common::error::HttpError;
use common::memory::workspace::{NodeKind, WorkspaceNode, effective_user_id, validate_name};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
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

            // Write empty .md to RustFS (best-effort; don't fail if RustFS is slow)
            let _ = state
                .workspace_content
                .write(&tenant.tenant_id, &node.virtual_path, "")
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
        let nodes = state
            .workspace_store
            .semantic_search_nodes(&tenant.tenant_id, user, &query, limit)
            .await
            .map_err(map_err)?;
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
        let seeded = state
            .workspace_store
            .is_tenant_seeded(&tenant.tenant_id)
            .await
            .unwrap_or(true); // on error, assume seeded to avoid repeated provision

        if !seeded {
            // Acquire the per-tenant single-flight lock before provisioning.
            let guard = {
                let mut guards = state.onboarding_guards.lock().unwrap();
                guards
                    .entry(tenant.tenant_id.as_str().to_owned())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                    .clone()
            };
            let _lock = guard.lock().await;

            // Re-check after acquiring lock (another request may have seeded).
            let still_unseeded = state
                .workspace_store
                .is_tenant_seeded(&tenant.tenant_id)
                .await
                .unwrap_or(true);

            if !still_unseeded {
                use agent_core::store::onboarding::{OnboardingOptions, TenantKind};
                let owner = tenant.user_id.as_deref().unwrap_or("__dev__");
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

                // Re-fetch after provisioning so the root folder appears immediately.
                let fresh = state
                    .workspace_store
                    .list_accessible_children(&tenant.tenant_id, user, None)
                    .await
                    .map_err(map_err)?;
                return Ok(Json(apply_cursor(fresh, q.after)));
            }
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

    // Write content to RustFS, then embed + index for semantic search (async).
    state
        .workspace_content
        .write(&tenant.tenant_id, &node.virtual_path, &body.content)
        .await
        .map_err(map_err)?;

    // Embed content chunks and upsert into Qdrant with tenant/owner context.
    {
        let content = body.content.clone();
        let tenant_id = tenant.tenant_id.clone();
        let owner_id = node.owner_id.clone();
        let node_id_str = id.to_string();
        let embedding_svc = Arc::clone(&state.embedding_service);
        let vector_store = Arc::clone(&state.vector_store);
        tokio::spawn(async move {
            const CHUNK: usize = 1500;
            let chunks: Vec<String> = content
                .chars()
                .collect::<Vec<_>>()
                .chunks(CHUNK)
                .map(|c| c.iter().collect::<String>())
                .collect();
            if let Ok(embeddings) = embedding_svc.embed_documents(chunks.clone()).await {
                for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                    let chunk_id = format!("{node_id_str}_{i}");
                    let _ = vector_store
                        .upsert_content_embedding_full(
                            &chunk_id,
                            &node_id_str,
                            i as i32,
                            chunk,
                            emb,
                            &tenant_id,
                            &owner_id,
                            &[],
                        )
                        .await;
                }
            }
        });
    }

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

    // Copy object to new key and delete old key when virtual_path changed.
    if let Some(old_path) = old_virtual_path
        && node.virtual_path != old_path
    {
        match state
            .workspace_content
            .read(&tenant.tenant_id, &old_path)
            .await
        {
            Ok(content) if !content.is_empty() => {
                if let Err(e) = state
                    .workspace_content
                    .write(&tenant.tenant_id, &node.virtual_path, &content)
                    .await
                {
                    tracing::warn!(error = %e, "move_node: failed to copy object to new key");
                } else {
                    let _ = state
                        .workspace_content
                        .delete(&tenant.tenant_id, &old_path)
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

#[derive(Deserialize)]
pub struct RenameBody {
    pub name: String,
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

    // Get the node first so we can clean up RustFS content
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // Best-effort RustFS cleanup for conversations
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

    state
        .realtime_service
        .publish_workspace_change(WorkspaceChangeEvent {
            op: "workspace.deleted".into(),
            tenant_id: tenant.tenant_id.to_string(),
            node_id: id.to_string(),
            kind: format!("{:?}", node.kind).to_lowercase(),
        })
        .await;

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers for presign routes ───────────────────────────────────────────────

fn presign_ttl_secs() -> i64 {
    std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(900)
        .min(3600)
}

fn presign_ttl() -> Duration {
    Duration::from_secs(presign_ttl_secs() as u64)
}

// ── Presigned URL endpoints ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PresignUploadBody {
    pub virtual_path: String,
    /// Accepted for forward compatibility — currently ignored by the presigner;
    /// clients SHOULD repeat the Content-Type on the PUT.
    #[allow(dead_code)]
    pub content_type: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct PresignUploadResponse {
    pub url: String,
    pub expires_at: String,
    pub virtual_path: String,
}

/// POST /v1/workspaces/:id/presign-upload — presigned PUT for direct browser → RustFS upload.
#[instrument(skip(state, tenant, body))]
pub async fn presign_upload(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<PresignUploadBody>,
) -> Result<Json<PresignUploadResponse>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.tenant_id, tenant.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }

    // Verify workspace node exists and is accessible
    let user = effective_user_id(tenant.user_id.as_deref());
    let _node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    // Check quota if size is provided
    if let Some(size) = body.size_bytes
        && let Err(e) = state
            .storage_quota
            .check(&tenant.tenant_id, &tenant.plan, size)
            .await
    {
        return Err(HttpError::validation(
            "size_bytes",
            format!("quota exceeded: {e}"),
        ));
    }

    let vp = VirtualPath::parse(&body.virtual_path)
        .map_err(|e| HttpError::validation("virtual_path", format!("{e}")))?;

    let storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;

    let url = storage
        .presign_workspace_put(&vp, presign_ttl())
        .await
        .map_err(|e| HttpError::agent(format!("presign PUT: {e}")))?;

    let ttl = presign_ttl_secs();
    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl)).to_rfc3339();

    Ok(Json(PresignUploadResponse {
        url: url.to_string(),
        expires_at,
        virtual_path: body.virtual_path,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PresignDownloadQuery {
    pub virtual_path: String,
}

#[derive(Serialize)]
pub struct PresignDownloadResponse {
    pub url: String,
    pub expires_at: String,
}

/// GET /v1/workspaces/:id/presign-download?virtual_path= — presigned GET for download.
#[instrument(skip(state, tenant))]
pub async fn presign_download(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Query(q): Query<PresignDownloadQuery>,
) -> Result<Json<PresignDownloadResponse>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let _node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let vp = VirtualPath::parse(&q.virtual_path)
        .map_err(|e| HttpError::validation("virtual_path", format!("{e}")))?;

    let storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;

    let url = storage
        .presign_workspace_get(&vp, presign_ttl(), None)
        .await
        .map_err(|e| HttpError::agent(format!("presign GET: {e}")))?;

    let ttl = presign_ttl_secs();
    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl)).to_rfc3339();

    Ok(Json(PresignDownloadResponse {
        url: url.to_string(),
        expires_at,
    }))
}

// ── Versioning endpoints ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct VersionEntry {
    pub version_id: String,
    pub last_modified: String,
    pub size: usize,
    pub is_current: bool,
}

/// GET /v1/workspaces/nodes/:id/versions — list object versions (requires versioning enabled).
#[instrument(skip(state, tenant))]
pub async fn list_versions(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
) -> Result<Json<Vec<VersionEntry>>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let admin = state
        .rustfs_admin
        .as_ref()
        .ok_or_else(|| HttpError::agent("RustFS admin client not configured"))?;

    let storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;

    let prefix = storage
        .workspace_s3_key(&node.virtual_path)
        .map_err(|e| HttpError::agent(format!("path: {e}")))?;

    let raw = admin
        .list_object_versions(&prefix)
        .await
        .map_err(|e| HttpError::agent(format!("list versions: {e}")))?;

    let versions: Vec<VersionEntry> = raw
        .into_iter()
        .map(
            |(version_id, last_modified, size, is_latest)| VersionEntry {
                version_id,
                last_modified,
                size: size as usize,
                is_current: is_latest,
            },
        )
        .collect();

    Ok(Json(versions))
}

#[derive(Deserialize)]
pub struct RestoreBody {
    pub version_id: String,
}

/// POST /v1/workspaces/nodes/:id/restore — restore a previous version.
#[instrument(skip(state, tenant, body))]
pub async fn restore_version(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(id): Path<Ulid>,
    Json(body): Json<RestoreBody>,
) -> Result<Json<WorkspaceNode>, HttpError> {
    let user = effective_user_id(tenant.user_id.as_deref());
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let admin = state
        .rustfs_admin
        .as_ref()
        .ok_or_else(|| HttpError::agent("RustFS admin client not configured"))?;

    // The version_id is a real S3 VersionId returned by list_object_versions.
    // Fetch the versioned bytes directly via ?versionId= query.
    let restore_storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;
    let object_key = restore_storage
        .workspace_s3_key(&node.virtual_path)
        .map_err(|e| HttpError::agent(format!("path: {e}")))?;
    let version_bytes = admin
        .get_object_version(&object_key, &body.version_id)
        .await
        .map_err(|e| HttpError::not_found(format!("version not found: {e}")))?;

    let content_str = String::from_utf8_lossy(&version_bytes).into_owned();

    state
        .workspace_content
        .write(&tenant.tenant_id, &node.virtual_path, &content_str)
        .await
        .map_err(map_err)?;

    let updated = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    Ok(Json(updated))
}
