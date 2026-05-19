/// File upload / download via RustFS (S3-compatible).
///
/// POST /v1/files          — multipart upload, returns download token (requires JWT)
/// GET  /v1/files/{token}  — stream file back (token-gated, 1h TTL; no JWT needed)
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    body::Body,
    extract::{Multipart, Path, State},
    response::Response,
};
use common::error::HttpError;
use object_store::{ObjectStore, path::Path as OsPath};
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Upload a file for the calling tenant.
#[utoipa::path(
    post,
    path = "/v1/files",
    request_body(content_type = "multipart/form-data", content = serde_json::Value),
    responses(
        (status = 200, description = "File uploaded, returns download token", body = serde_json::Value),
        (status = 429, description = "Rate limit exceeded"),
    ),
    security(("bearer_auth" = [])),
    tag = "files",
)]
#[instrument(skip(state, tenant, multipart))]
pub async fn upload(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, HttpError> {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        return Err(HttpError::rate_limit(None));
    }

    let store = state
        .file_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("file storage not configured (S3_ENDPOINT missing?)"))?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| HttpError::validation("file", format!("multipart error: {e}")))?
        .ok_or_else(|| HttpError::validation("file", "no file field found in multipart request"))?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.bin", Uuid::new_v4()));

    let data = field
        .bytes()
        .await
        .map_err(|e| HttpError::validation("file", format!("read error: {e}")))?;

    let object_key = format!(
        "{}{}/{}",
        tenant.0.storage_prefix(),
        Uuid::new_v4(),
        filename
    );
    let os_path = OsPath::from(object_key.as_str());
    let size = data.len();

    store
        .put(&os_path, data.into())
        .await
        .map_err(|e| HttpError::agent(format!("storage write error: {e}")))?;

    // Issue a time-limited download token bound to this tenant
    let token = Uuid::new_v4().to_string();
    {
        let mut tokens = state.presigned_tokens.lock().unwrap();
        tokens.insert(
            token.clone(),
            (
                object_key,
                std::time::Instant::now(),
                std::time::Duration::from_secs(3600),
                tenant.0.tenant_id.to_string(),
            ),
        );
    }

    Ok(Json(json!({
        "id": token,
        "filename": filename,
        "size": size,
        "tenant_id": tenant.0.tenant_id,
        "download_url": format!("/v1/files/{token}")
    })))
}

/// Download a file by token.
///
/// The UUID token is the credential — no JWT required. Possession of the token
/// is equivalent to a presigned URL: the token has a 1-hour TTL and is
/// unguessable (UUID v4, 122 bits of entropy).
pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Response, HttpError> {
    let store = state
        .file_store
        .as_ref()
        .ok_or_else(|| HttpError::agent("file storage not configured"))?;

    let object_key = {
        let tokens = state.presigned_tokens.lock().unwrap();
        let (key, created, ttl, _tenant_id) = tokens
            .get(&token)
            .ok_or_else(|| HttpError::not_found("download token"))?;
        if created.elapsed() > *ttl {
            return Err(HttpError::not_found("download token (expired)"));
        }
        key.clone()
    };

    let os_path = OsPath::from(object_key.as_str());
    let result = store
        .get(&os_path)
        .await
        .map_err(|e| HttpError::not_found(format!("object not found: {e}")))?;

    let bytes = result
        .bytes()
        .await
        .map_err(|e| HttpError::agent(format!("read error: {e}")))?;

    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("content-length", bytes.len().to_string())
        .body(Body::from(bytes))
        .expect("response build failed"))
}
