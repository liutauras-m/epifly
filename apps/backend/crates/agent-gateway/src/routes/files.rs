//! File storage routes — direct upload via presigned URLs only.
//!
//! The old UUID-token download shim (`GET /v1/files/{token}`) is removed.
//! All uploads/downloads now use real S3 presigned URLs signed with
//! per-tenant credentials.
//!
//! POST /v1/files/upload-url  — issue a presigned PUT URL for direct browser → RustFS upload
//! POST /v1/files/confirm     — record metadata after upload completes

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{presign_put, presign_tmp_put};

fn per_tenant_iam() -> bool {
    std::env::var("RUSTFS_PER_TENANT_IAM").as_deref() != Ok("off")
}
use axum::{Extension, Json, extract::State, http::StatusCode};
use chrono::Utc;
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

#[derive(Deserialize)]
pub struct PresignUploadBody {
    pub virtual_path: String,
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
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }

    // Check storage quota
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

    let creds = state
        .cred_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("credential store not configured"))?
        .load(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("load creds: {e}")))?;

    let creds = match creds {
        Some(c) => c,
        None if per_tenant_iam() => {
            return Err(HttpError::agent("IAM credentials not provisioned for tenant"));
        }
        None => agent_core::StorageCreds {
            access_key: std::env::var("RUSTFS_ROOT_ACCESS_KEY")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            secret_key: std::env::var("RUSTFS_ROOT_SECRET_KEY")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            created_at: 0,
            bucket: None,
        },
    };

    let endpoint = std::env::var("S3_ENDPOINT")
        .unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET")
        .unwrap_or_else(|_| "workspace".into());

    let url = presign_put(
        &tenant.0.tenant_id,
        &body.virtual_path,
        &creds,
        &endpoint,
        &bucket,
        None,
    )
    .await
    .map_err(|e| HttpError::agent(format!("presign PUT: {e}")))?;

    let ttl_secs: i64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl_secs)).to_rfc3339();

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
    let creds = state
        .cred_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("credential store not configured"))?
        .load(&tenant.0.tenant_id)
        .await
        .map_err(|e| HttpError::agent(format!("load creds: {e}")))?;

    let creds = match creds {
        Some(c) => c,
        None if per_tenant_iam() => {
            return Err(HttpError::agent("IAM credentials not provisioned for tenant"));
        }
        None => agent_core::StorageCreds {
            access_key: std::env::var("RUSTFS_ROOT_ACCESS_KEY")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            secret_key: std::env::var("RUSTFS_ROOT_SECRET_KEY")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_else(|_| "rustfsadmin".into()),
            created_at: 0,
            bucket: None,
        },
    };

    let endpoint = std::env::var("S3_ENDPOINT")
        .unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET")
        .unwrap_or_else(|_| "workspace".into());

    let url = agent_core::presign_get(
        &tenant.0.tenant_id,
        &q.virtual_path,
        &creds,
        &endpoint,
        &bucket,
        None,
    )
    .await
    .map_err(|e| HttpError::agent(format!("presign GET: {e}")))?;

    let ttl_secs: i64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    let expires_at = (Utc::now() + chrono::Duration::seconds(ttl_secs)).to_rfc3339();

    Ok(Json(PresignDownloadResponse {
        url: url.to_string(),
        expires_at,
    }))
}
