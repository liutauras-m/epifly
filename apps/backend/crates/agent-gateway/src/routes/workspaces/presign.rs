use super::errors::map_err;
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::VirtualPath;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use common::error::HttpError;
use common::memory::workspace::{WorkspaceNode, effective_user_id};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;
use ulid::Ulid;

// ── TTL + content-type helpers ───────────────────────────────────────────────

pub(super) fn presign_ttl_secs() -> i64 {
    std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(900)
        .min(3600)
}

pub(super) fn presign_ttl() -> Duration {
    Duration::from_secs(presign_ttl_secs() as u64)
}

pub(super) const HARD_MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;

pub(super) fn allowed_presign_content_type(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(
        essence.as_str(),
        "application/json"
            | "application/pdf"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "image/gif"
            | "image/jpeg"
            | "image/png"
            | "image/webp"
            | "text/csv"
            | "text/markdown"
            | "text/plain"
    )
}

// ── Path resolution ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub(super) enum PresignPathError {
    InvalidStoredPath(String),
    MissingForFolder,
    InvalidRequestedPath(String),
    OutsideNode,
}

impl PresignPathError {
    pub(super) fn into_http(self) -> HttpError {
        match self {
            PresignPathError::InvalidStoredPath(message) => {
                HttpError::agent(format!("stored workspace node path is invalid: {message}"))
            }
            PresignPathError::MissingForFolder => {
                HttpError::validation("virtual_path", "virtual_path is required for folder nodes")
            }
            PresignPathError::InvalidRequestedPath(message) => {
                HttpError::validation("virtual_path", message)
            }
            PresignPathError::OutsideNode => HttpError::forbidden("virtual_path outside node"),
        }
    }
}

use common::memory::workspace::NodeKind;

pub(super) fn resolve_presign_path(
    node: &WorkspaceNode,
    requested: Option<&str>,
) -> Result<VirtualPath, PresignPathError> {
    let node_path = VirtualPath::parse(&node.virtual_path)
        .map_err(|e| PresignPathError::InvalidStoredPath(e.to_string()))?;

    match node.kind {
        NodeKind::Conversation | NodeKind::File => Ok(node_path),
        NodeKind::Folder => {
            let raw = requested.ok_or(PresignPathError::MissingForFolder)?;
            let requested_path = VirtualPath::parse(raw)
                .map_err(|e| PresignPathError::InvalidRequestedPath(e.to_string()))?;
            if !requested_path.is_strict_child_of(&node_path) {
                return Err(PresignPathError::OutsideNode);
            }
            Ok(requested_path)
        }
    }
}

// ── Request / response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PresignUploadBody {
    /// Legacy-only for file/conversation nodes; the server derives their content
    /// path from the node. Required for folder nodes and must be a strict child.
    pub virtual_path: Option<String>,
    pub content_type: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct PresignUploadResponse {
    pub url: String,
    pub expires_at: String,
    pub virtual_path: String,
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

// ── Handlers ─────────────────────────────────────────────────────────────────

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
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let content_type = body
        .content_type
        .as_deref()
        .ok_or_else(|| HttpError::validation("content_type", "content_type is required"))?;
    if !allowed_presign_content_type(content_type) {
        return Err(HttpError::validation(
            "content_type",
            "unsupported content type",
        ));
    }

    let size = body
        .size_bytes
        .ok_or_else(|| HttpError::validation("size_bytes", "size_bytes is required"))?;
    let max_upload = tenant
        .plan
        .limits()
        .max_upload_bytes
        .min(HARD_MAX_UPLOAD_BYTES);
    if size > max_upload {
        return Err(HttpError::validation(
            "size_bytes",
            format!("upload exceeds maximum size of {max_upload} bytes"),
        ));
    }

    // Check quota if size is provided
    if let Err(e) = state
        .storage_quota
        .check(&tenant.tenant_id, &tenant.plan, size)
        .await
    {
        return Err(HttpError::validation(
            "size_bytes",
            format!("quota exceeded: {e}"),
        ));
    }

    let vp = resolve_presign_path(&node, body.virtual_path.as_deref())
        .map_err(PresignPathError::into_http)?;

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
        virtual_path: vp.to_string(),
    }))
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
    let node = state
        .workspace_store
        .get_accessible_node(&tenant.tenant_id, user, id)
        .await
        .map_err(map_err)?;

    let vp = resolve_presign_path(&node, Some(q.virtual_path.as_str()))
        .map_err(PresignPathError::into_http)?;

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
