use super::content_indexing::enqueue_reindex;
use super::errors::{map_content_err, map_err};
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use common::error::HttpError;
use common::memory::workspace::{WorkspaceNode, effective_user_id};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

#[derive(Serialize)]
pub struct VersionEntry {
    pub version_id: String,
    pub last_modified: String,
    pub size: usize,
    pub is_current: bool,
}

#[derive(Deserialize)]
pub struct RestoreBody {
    pub version_id: String,
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

    let (key, legacy) = match &node.object_key {
        Some(ok) => (ok.as_str(), Some(node.virtual_path.as_str())),
        None => (node.virtual_path.as_str(), None),
    };
    state
        .workspace_content
        .write(&tenant.tenant_id, key, legacy, &content_str)
        .await
        .map_err(map_content_err)?;

    // Re-index the restored content via the durable job — same path as patch_content.
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
