//! Device-token management for shell clients.
//!
//! `POST /admin/devices`        — issue a fresh token (requires PLATFORM_ADMIN_TOKEN bearer)
//! `GET /admin/devices`         — list active tokens
//! `DELETE /admin/devices/{id}` — revoke a token
//!
//! Tokens are stored in-memory (via AppState.device_tokens). They are lost on restart,
//! which is acceptable for the browser shell feature.

use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[allow(clippy::result_large_err)]
pub(crate) fn require_shell_feature(enabled: bool) -> Result<(), HttpError> {
    if enabled {
        Ok(())
    } else {
        Err(HttpError::not_found("browser shell feature not enabled"))
    }
}

#[allow(clippy::result_large_err)]
fn require_platform_admin(headers: &HeaderMap) -> Result<(), HttpError> {
    let expected = std::env::var("PLATFORM_ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty() {
        return Ok(());
    }
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if bearer == expected {
        Ok(())
    } else {
        Err(HttpError::auth("invalid platform admin token"))
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IssueDeviceRequest {
    pub tenant_id: String,
    pub device_label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IssueDeviceResponse {
    pub id: String,
    pub token: String,
    pub device_label: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DeviceSummary {
    pub id: String,
    pub tenant_id: String,
    pub device_label: String,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
}

/// In-memory device token record.
#[derive(Debug, Clone)]
pub struct DeviceToken {
    pub id: String,
    pub tenant_id: String,
    pub device_label: String,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[utoipa::path(
    post,
    path = "/admin/devices",
    request_body = IssueDeviceRequest,
    responses(
        (status = 201, description = "Device token issued", body = IssueDeviceResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Browser shell feature not enabled"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn issue_device(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IssueDeviceRequest>,
) -> Result<impl IntoResponse, HttpError> {
    require_shell_feature(state.browser_shell_enabled)?;
    require_platform_admin(&headers)?;

    let raw: [u8; 32] = rand::random();
    let plaintext = hex::encode(raw);
    let hash = blake3::hash(plaintext.as_bytes());
    let token_hash = hex::encode(hash.as_bytes());
    let id = Uuid::new_v4().to_string();

    state.device_tokens.insert(
        token_hash,
        DeviceToken {
            id: id.clone(),
            tenant_id: req.tenant_id,
            device_label: req.device_label.clone(),
            created_at: Utc::now(),
            last_seen: None,
            revoked: false,
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(IssueDeviceResponse {
            id,
            token: plaintext,
            device_label: req.device_label,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/admin/devices",
    responses(
        (status = 200, description = "List of active device tokens", body = Vec<DeviceSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Browser shell feature not enabled"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn list_devices(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceSummary>>, HttpError> {
    require_shell_feature(state.browser_shell_enabled)?;
    require_platform_admin(&headers)?;

    let mut result: Vec<DeviceSummary> = state
        .device_tokens
        .iter()
        .filter(|e| !e.value().revoked)
        .map(|e| {
            let t = e.value();
            DeviceSummary {
                id: t.id.clone(),
                tenant_id: t.tenant_id.clone(),
                device_label: t.device_label.clone(),
                created_at: t.created_at,
                last_seen: t.last_seen,
            }
        })
        .collect();
    result.sort_by_key(|d| std::cmp::Reverse(d.created_at));
    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/admin/devices/{id}",
    params(
        ("id" = Uuid, Path, description = "Device token UUID to revoke"),
    ),
    responses(
        (status = 204, description = "Token revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Browser shell feature not enabled"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn revoke_device(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, HttpError> {
    require_shell_feature(state.browser_shell_enabled)?;
    require_platform_admin(&headers)?;

    for mut entry in state.device_tokens.iter_mut() {
        if entry.value().id == id {
            entry.value_mut().revoked = true;
            return Ok(StatusCode::NO_CONTENT);
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Validates a device token and bumps `last_seen`. Returns `tenant_id` on success.
pub async fn validate_device_token(
    state: &AppState,
    token: &str,
) -> anyhow::Result<Option<String>> {
    let hash = blake3::hash(token.as_bytes());
    let token_hash = hex::encode(hash.as_bytes());

    if let Some(mut record) = state.device_tokens.get_mut(&token_hash)
        && !record.revoked
    {
        record.last_seen = Some(Utc::now());
        return Ok(Some(record.tenant_id.clone()));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flag_off_returns_not_found() {
        let err = require_shell_feature(false).unwrap_err();
        assert_eq!(err.status.as_u16(), 404);
    }

    #[test]
    fn feature_flag_on_passes() {
        assert!(require_shell_feature(true).is_ok());
    }

    #[test]
    fn token_hash_is_deterministic() {
        let a = blake3::hash(b"test_token");
        let b = blake3::hash(b"test_token");
        assert_eq!(a, b);
    }
}
