//! Multipart upload endpoints for large files (> 16 MiB).
//!
//! POST /v1/uploads/initiate              — start a multipart upload
//! POST /v1/uploads/:id/parts/:n/presign  — presigned URL for part N
//! POST /v1/uploads/:id/complete          — complete multipart
//! POST /v1/uploads/:id/abort             — abort and clean up

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{StorageCreds, presign_tmp_put};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use common::error::HttpError;
use object_store::path::Path as OsPath;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

fn per_tenant_iam() -> bool {
    std::env::var("RUSTFS_PER_TENANT_IAM").as_deref() != Ok("off")
}

async fn tenant_creds(state: &AppState, tenant_id: &str) -> Result<StorageCreds, HttpError> {
    match state
        .cred_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("credential store not configured"))?
        .load(tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("load creds: {e}")))?
    {
        Some(c) => Ok(c),
        None if per_tenant_iam() => Err(HttpError::agent(
            "IAM credentials not provisioned for tenant",
        )),
        None => Ok(StorageCreds {
            access_key: std::env::var("RUSTFS_ROOT_ACCESS_KEY")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            secret_key: std::env::var("RUSTFS_ROOT_SECRET_KEY")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            created_at: 0,
        }),
    }
}

#[derive(Deserialize)]
pub struct InitiateBody {
    pub filename: String,
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
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }

    if let Some(size) = body.size_bytes {
        if let Err(e) = state
            .storage_quota
            .check(&tenant.0.tenant_id, &tenant.0.plan, size)
            .await
        {
            return Err(HttpError::validation("size_bytes", format!("quota: {e}")));
        }
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

    let creds = tenant_creds(&state, &tenant.0.tenant_id).await?;
    let endpoint = std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "workspace".into());

    let url = presign_tmp_put(
        &tenant.0.tenant_id,
        &upload_id,
        filename,
        &creds,
        &endpoint,
        &bucket,
    )
    .await
    .map_err(|e| HttpError::agent(format!("presign part: {e}")))?;

    let ttl_secs: i64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl_secs)).to_rfc3339();

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

/// POST /v1/uploads/:upload_id/complete — finalize multipart upload.
///
/// The client must have already PUT all parts to their presigned URLs.
/// This endpoint reads staged parts from `tenants/{id}/uploads/tmp/{upload_id}/`,
/// concatenates them in sort order, writes to the permanent workspace path,
/// deletes the staging objects, and returns the real byte count.
#[instrument(skip(state, tenant, body))]
pub async fn complete(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(upload_id): Path<String>,
    Json(body): Json<CompleteBody>,
) -> Result<Json<CompleteResponse>, HttpError> {
    let file_store = state
        .file_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("file store not configured"))?;

    let staging_prefix = OsPath::from(format!(
        "tenants/{}/uploads/tmp/{}/",
        tenant.0.tenant_id, upload_id
    ));

    let listed = file_store
        .list_with_delimiter(Some(&staging_prefix))
        .await
        .map_err(|e| HttpError::agent(format!("list staged parts: {e}")))?;

    if listed.objects.is_empty() {
        return Err(HttpError::not_found("no staged parts found for upload"));
    }

    let mut objects = listed.objects;
    objects.sort_by(|a, b| a.location.as_ref().cmp(b.location.as_ref()));

    let mut combined: Vec<u8> = Vec::new();
    for obj in &objects {
        let result = file_store
            .get(&obj.location)
            .await
            .map_err(|e| HttpError::agent(format!("read staged part: {e}")))?;
        let bytes = result
            .bytes()
            .await
            .map_err(|e| HttpError::agent(format!("read staged bytes: {e}")))?;
        combined.extend_from_slice(&bytes);
    }

    let size_bytes = combined.len() as u64;
    let content = String::from_utf8_lossy(&combined).into_owned();

    state
        .workspace_content
        .write(&tenant.0.tenant_id, &body.destination_path, &content)
        .await
        .map_err(|e| HttpError::agent(format!("write to destination: {e}")))?;

    for obj in &objects {
        let _ = file_store.delete(&obj.location).await;
    }

    state.storage_quota.invalidate(&tenant.0.tenant_id);

    Ok(Json(CompleteResponse {
        virtual_path: body.destination_path,
        size_bytes,
    }))
}

/// POST /v1/uploads/:upload_id/abort — abort and discard staged parts.
#[instrument(skip(state, tenant))]
pub async fn abort(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(upload_id): Path<String>,
) -> StatusCode {
    // Parts in uploads/tmp/{upload_id}/ will be cleaned up by lifecycle rules (24h TTL).
    tracing::info!(
        tenant_id = %tenant.0.tenant_id,
        upload_id,
        "multipart upload aborted (lifecycle will clean up tmp parts)"
    );
    StatusCode::NO_CONTENT
}
