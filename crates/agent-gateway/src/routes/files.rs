/// File upload / download via MinIO (S3-compatible).
///
/// POST /v1/files          — multipart upload, returns download token
/// GET  /v1/files/{token}  — stream file back (token-gated, 1h TTL)
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Response,
};
use object_store::{ObjectStore, path::Path as OsPath};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Upload a file for the calling tenant.
#[instrument(skip(state, tenant, multipart))]
pub async fn upload(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "rate limit exceeded"})),
        ));
    }

    let store = state.file_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "file storage not configured (MINIO_ENDPOINT missing?)"})),
        )
    })?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("multipart error: {e}")})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no file field found in multipart request"})),
            )
        })?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.bin", Uuid::new_v4()));

    let data = field.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("read error: {e}")})),
        )
    })?;

    let object_key = format!(
        "{}{}/{}",
        tenant.0.storage_prefix(),
        Uuid::new_v4(),
        filename
    );
    let os_path = OsPath::from(object_key.as_str());
    let size = data.len();

    store.put(&os_path, data.into()).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("storage write error: {e}")})),
        )
    })?;

    // Issue a time-limited download token
    let token = Uuid::new_v4().to_string();
    {
        let mut tokens = state.presigned_tokens.lock().unwrap();
        tokens.insert(
            token.clone(),
            (
                object_key,
                std::time::Instant::now(),
                std::time::Duration::from_secs(3600),
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
pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let store = state.file_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "file storage not configured"})),
        )
    })?;

    let object_key = {
        let tokens = state.presigned_tokens.lock().unwrap();
        let (key, created, ttl) = tokens.get(&token).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "token not found or expired"})),
            )
        })?;
        if created.elapsed() > *ttl {
            return Err((StatusCode::GONE, Json(json!({"error": "token expired"}))));
        }
        key.clone()
    };

    let os_path = OsPath::from(object_key.as_str());
    let result = store.get(&os_path).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("object not found: {e}")})),
        )
    })?;

    let bytes = result.bytes().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("read error: {e}")})),
        )
    })?;

    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("content-length", bytes.len().to_string())
        .body(Body::from(bytes))
        .expect("response build failed"))
}
