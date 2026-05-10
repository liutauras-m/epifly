//! Device-token management for shell clients.
//!
//! `POST /admin/devices`        — issue a fresh token (requires PLATFORM_ADMIN_TOKEN bearer)
//! `GET /admin/devices`         — list active tokens
//! `DELETE /admin/devices/{id}` — revoke a token

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
use sqlx::PgPool;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[allow(clippy::result_large_err)]
fn pool(state: &AppState) -> Result<&PgPool, HttpError> {
    state
        .pool
        .as_ref()
        .ok_or_else(|| HttpError::internal("no database pool", None))
}

#[allow(clippy::result_large_err)]
pub(crate) fn require_shell_feature() -> Result<(), HttpError> {
    if std::env::var("CONUSAI_FEATURE_BROWSER_SHELL").as_deref() == Ok("1") {
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
    /// Tenant this device belongs to.
    pub tenant_id: String,
    /// Human-readable label for the device.
    pub device_label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IssueDeviceResponse {
    /// UUID of the newly created device token record.
    pub id: String,
    /// Plaintext token — shown once; store it securely.
    pub token: String,
    pub device_label: String,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DeviceSummary {
    pub id: Option<String>,
    pub tenant_id: String,
    pub device_label: String,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
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
    require_shell_feature()?;
    require_platform_admin(&headers)?;
    let pool = pool(&state)?;

    let raw: [u8; 32] = rand::random();
    let plaintext = hex::encode(raw);
    let hash = blake3::hash(plaintext.as_bytes());
    let hash_bytes = hash.as_bytes().to_vec();

    let id: Option<String> = sqlx::query_scalar(
        "INSERT INTO device_tokens (tenant_id, device_label, token_hash) \
         VALUES ($1, $2, $3) RETURNING id::text",
    )
    .bind(&req.tenant_id)
    .bind(&req.device_label)
    .bind(&hash_bytes)
    .fetch_one(pool)
    .await
    .map_err(|e| HttpError::internal(e.to_string(), None))?;

    Ok((
        StatusCode::CREATED,
        Json(IssueDeviceResponse {
            id: id.unwrap_or_default(),
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
    require_shell_feature()?;
    require_platform_admin(&headers)?;
    let pool = pool(&state)?;

    let rows: Vec<DeviceSummary> = sqlx::query_as(
        "SELECT id::text as id, tenant_id, device_label, created_at, last_seen \
         FROM device_tokens WHERE revoked_at IS NULL ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| HttpError::internal(e.to_string(), None))?;

    Ok(Json(rows))
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
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpError> {
    require_shell_feature()?;
    require_platform_admin(&headers)?;
    let pool = pool(&state)?;

    sqlx::query("UPDATE device_tokens SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| HttpError::internal(e.to_string(), None))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Validates a device token and bumps `last_seen`. Returns `tenant_id` on success.
pub async fn validate_device_token(pool: &PgPool, token: &str) -> anyhow::Result<Option<String>> {
    // Note: intentionally raw sqlx (not query!) to avoid compile-time DATABASE_URL requirement.
    let hash = blake3::hash(token.as_bytes());
    let hash_bytes = hash.as_bytes().to_vec();

    let tenant_id: Option<String> = sqlx::query_scalar(
        "UPDATE device_tokens SET last_seen = now() \
         WHERE token_hash = $1 AND revoked_at IS NULL \
         RETURNING tenant_id",
    )
    .bind(&hash_bytes)
    .fetch_optional(pool)
    .await?;

    Ok(tenant_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flag_off_returns_not_found() {
        // SAFETY: test-only; single-threaded test binary.
        unsafe { std::env::remove_var("CONUSAI_FEATURE_BROWSER_SHELL") };
        let err = require_shell_feature().unwrap_err();
        assert_eq!(err.status.as_u16(), 404);
    }

    #[test]
    fn feature_flag_on_passes() {
        // SAFETY: test-only; single-threaded test binary.
        unsafe { std::env::set_var("CONUSAI_FEATURE_BROWSER_SHELL", "1") };
        let result = require_shell_feature();
        unsafe { std::env::remove_var("CONUSAI_FEATURE_BROWSER_SHELL") };
        assert!(result.is_ok());
    }

    #[test]
    fn token_hash_is_deterministic() {
        let a = blake3::hash(b"test_token");
        let b = blake3::hash(b"test_token");
        assert_eq!(a, b);
    }

    #[test]
    fn different_tokens_produce_different_hashes() {
        let a = blake3::hash(b"token_a");
        let b = blake3::hash(b"token_b");
        assert_ne!(a, b);
    }
}
