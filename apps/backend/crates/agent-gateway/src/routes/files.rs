//! File storage routes — direct upload/download via presigned URLs.
//!
//! POST /v1/files/upload-url  — issue a presigned PUT URL for direct browser → RustFS upload
//! GET  /v1/files/download-url?virtual_path= — issue a presigned GET URL

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::VirtualPath;
use axum::{Extension, Json, extract::State};
use chrono::Utc;
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tracing::instrument;

fn presign_ttl() -> Duration {
    let secs: u64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    Duration::from_secs(secs)
}

#[derive(Deserialize)]
pub struct PresignUploadBody {
    pub virtual_path: String,
    /// Accepted for forward compatibility — currently ignored by the presigner;
    /// clients SHOULD send the same Content-Type when PUT-ing.
    #[allow(dead_code)]
    pub content_type: Option<String>,
    pub size_bytes: u64,
}

#[derive(Serialize)]
pub struct PresignUploadResponse {
    pub url: String,
    pub expires_at: String,
    pub virtual_path: String,
}

/// POST /v1/files/upload-url — issue a presigned PUT URL for direct upload to RustFS.
#[instrument(skip(state, tenant, body))]
pub async fn presign_upload(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(body): Json<PresignUploadBody>,
) -> Result<Json<PresignUploadResponse>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }

    if let Err(e) = state
        .storage_quota
        .check(&tenant.0.tenant_id, &tenant.0.plan, body.size_bytes)
        .await
    {
        return Err(HttpError::validation(
            "size_bytes",
            format!("quota exceeded: {e}"),
        ));
    }

    let factory = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("storage not configured"))?;

    let storage = factory
        .for_tenant(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage: {e}")))?;

    let vp = VirtualPath::parse(&body.virtual_path)
        .map_err(|e| HttpError::validation("virtual_path", e.to_string()))?;

    let ttl = presign_ttl();
    let url = storage
        .presign_workspace_put(&vp, ttl)
        .await
        .map_err(|e| HttpError::agent(format!("presign PUT: {e}")))?;

    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl.as_secs() as i64)).to_rfc3339();

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

/// GET /v1/files/download-url?virtual_path= — issue a presigned GET URL.
#[instrument(skip(state, tenant), fields(tenant_id = %tenant.0.tenant_id))]
pub async fn presign_download(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    axum::extract::Query(q): axum::extract::Query<PresignDownloadQuery>,
) -> Result<Json<PresignDownloadResponse>, HttpError> {
    let factory = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("storage not configured"))?;

    let storage = factory
        .for_tenant(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage: {e}")))?;

    let vp = VirtualPath::parse(&q.virtual_path)
        .map_err(|e| HttpError::validation("virtual_path", e.to_string()))?;

    let ttl = presign_ttl();
    let url = storage
        .presign_workspace_get(&vp, ttl, None)
        .await
        .map_err(|e| HttpError::agent(format!("presign GET: {e}")))?;

    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl.as_secs() as i64)).to_rfc3339();

    Ok(Json(PresignDownloadResponse {
        url: url.to_string(),
        expires_at,
    }))
}
