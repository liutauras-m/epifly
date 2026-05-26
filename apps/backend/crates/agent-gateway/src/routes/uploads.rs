//! Multipart upload endpoints for large files (> 16 MiB).
//!
//! POST /v1/uploads/initiate              — start a multipart upload
//! POST /v1/uploads/:id/parts/:n/presign  — presigned URL for part N
//! POST /v1/uploads/:id/complete          — complete multipart (streaming, no RAM buffering)
//! POST /v1/uploads/:id/abort             — abort and clean up

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::VirtualPath;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;
use ulid::Ulid;

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

#[derive(Deserialize)]
pub struct InitiateBody {
    pub filename: String,
    /// Accepted for forward compatibility — currently ignored by the initiator;
    /// clients SHOULD repeat it on each part PUT.
    #[allow(dead_code)]
    pub content_type: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct InitiateResponse {
    pub upload_id: String,
    pub filename: String,
}

/// POST /v1/uploads/initiate — start a multipart upload session.
#[instrument(skip(state, tenant, body))]
pub async fn initiate(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(body): Json<InitiateBody>,
) -> Result<Json<InitiateResponse>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.limits().rate_limit_rpm)
    {
        return Err(HttpError::rate_limit(None));
    }

    if let Some(size) = body.size_bytes
        && let Err(e) = state
            .storage_quota
            .check(&tenant.0.tenant_id, &tenant.0.plan, size)
            .await
    {
        return Err(HttpError::validation("size_bytes", format!("quota: {e}")));
    }

    let upload_id = Ulid::new().to_string();
    Ok(Json(InitiateResponse {
        upload_id,
        filename: body.filename,
    }))
}

#[derive(Serialize)]
pub struct PartPresignResponse {
    pub url: String,
    pub expires_at: String,
}

/// POST /v1/uploads/:upload_id/parts/:n/presign — presigned URL for uploading part N.
#[instrument(skip(state, tenant))]
pub async fn presign_part(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path((upload_id, _part_n)): Path<(String, u32)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<PartPresignResponse>, HttpError> {
    let filename = body
        .get("filename")
        .and_then(|v| v.as_str())
        .unwrap_or("part.bin");

    let storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;

    let url = storage
        .presign_staging_put(&upload_id, filename, presign_ttl())
        .await
        .map_err(|e| HttpError::agent(format!("presign part: {e}")))?;

    let expires_at = (Utc::now() + chrono::Duration::seconds(presign_ttl_secs())).to_rfc3339();

    Ok(Json(PartPresignResponse {
        url: url.to_string(),
        expires_at,
    }))
}

#[derive(Deserialize)]
pub struct CompletePart {
    pub n: u32,
    pub etag: String,
}

#[derive(Deserialize)]
pub struct CompleteBody {
    pub parts: Vec<CompletePart>,
    pub destination_path: String,
}

#[derive(Serialize)]
pub struct CompleteResponse {
    pub virtual_path: String,
    pub size_bytes: u64,
}

/// POST /v1/uploads/:upload_id/complete — finalize multipart upload (streaming).
#[instrument(skip(state, tenant, body))]
pub async fn complete(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(upload_id): Path<String>,
    Json(body): Json<CompleteBody>,
) -> Result<Json<CompleteResponse>, HttpError> {
    let dest_vp = VirtualPath::parse(&body.destination_path)
        .map_err(|e| HttpError::validation("destination_path", format!("{e}")))?;

    let storage = state
        .tenant_storage
        .as_ref()
        .ok_or_else(|| HttpError::agent("tenant storage not configured"))?
        .for_tenant(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("storage for tenant: {e}")))?;

    let parts: Vec<agent_core::CompletedPart> = body
        .parts
        .iter()
        .map(|p| agent_core::CompletedPart {
            n: p.n,
            etag: p.etag.clone(),
        })
        .collect();

    let result = storage
        .finalize_staged_upload(&upload_id, &parts, &dest_vp)
        .await
        .map_err(|e| HttpError::agent(format!("finalize upload: {e}")))?;

    state.storage_quota.invalidate(&tenant.0.tenant_id);

    Ok(Json(CompleteResponse {
        virtual_path: result.virtual_path.to_string(),
        size_bytes: result.size_bytes,
    }))
}

/// POST /v1/uploads/:upload_id/abort — abort and discard staged parts.
#[instrument(skip(_state, tenant))]
pub async fn abort(
    State(_state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(upload_id): Path<String>,
) -> StatusCode {
    tracing::info!(
        tenant_id = %tenant.0.tenant_id,
        upload_id,
        "multipart upload aborted (lifecycle will clean up tmp parts)"
    );
    StatusCode::NO_CONTENT
}
