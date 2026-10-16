//! File download endpoint.
//! GET /ui/files/download?key={object_key}
//!
//! Authenticated like every `/ui/*` route. The object is read with the caller's
//! own tenant storage credentials and only when the key belongs to the tenant —
//! never with the root keys.

use crate::auth::ResolvedTenant;
use crate::state::AppState;
use agent_core::StorageError;
use agent_core::capabilities::object_source::validate_object_key;
use axum::{
    Extension,
    body::Body,
    extract::{Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub key: String,
}

pub async fn ui_download(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Query(q): Query<DownloadQuery>,
) -> Response {
    if validate_object_key(&q.key).is_err() {
        return err(StatusCode::BAD_REQUEST, "invalid object key");
    }
    let Some(factory) = state.tenant_storage.as_ref() else {
        return err(
            StatusCode::SERVICE_UNAVAILABLE,
            "file storage not configured",
        );
    };
    let storage = match factory.for_tenant(tenant.0.tenant_id.as_str()).await {
        Ok(s) => s,
        Err(StorageError::MissingTenantCreds) => {
            return err(StatusCode::NOT_FOUND, "object not found");
        }
        Err(_) => return err(StatusCode::SERVICE_UNAVAILABLE, "storage unavailable"),
    };
    let bytes = match storage.get_owned_object(&q.key).await {
        Ok(b) => b,
        Err(StorageError::Forbidden) => {
            return err(
                StatusCode::FORBIDDEN,
                "object does not belong to your tenant",
            );
        }
        Err(StorageError::NotFound) => return err(StatusCode::NOT_FOUND, "object not found"),
        Err(_) => return err(StatusCode::BAD_GATEWAY, "storage read failed"),
    };

    let filename: String = q
        .key
        .rsplit('/')
        .next()
        .unwrap_or("file")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
        ],
        Body::from(bytes),
    )
        .into_response()
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, msg.to_string()).into_response()
}
