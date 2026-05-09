//! Super-admin REST API for managing capabilities at runtime.
//!
//! All routes require `Authorization: Bearer <jwt>` with `role = "super_admin"`.
//! They are protected by the `require_super_admin_jwt` middleware layer.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{
    CapabilitySummary, CreateCapabilityRequest, TestInvokeRequest, UpdateCapabilityRequest,
    ValidationReport, RegisteredToolValidator,
};
use axum::{
    Extension,
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetEnabledPayload {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ValidationResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<ValidationReport> for ValidationResponse {
    fn from(r: ValidationReport) -> Self {
        Self {
            valid: r.ok(),
            errors: r.errors.iter().map(|e| e.to_string()).collect(),
            warnings: r.warnings,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// List all registered capabilities (enabled + disabled).
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
) -> Json<Vec<CapabilitySummary>> {
    Json(state.tool_admin.list())
}

/// Get a single capability by name.
pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.get(&name) {
        Some(c) => (StatusCode::OK, Json(c)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Get the raw TOML manifest for a capability.
pub async fn get_manifest(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.get_manifest_toml(&name) {
        Ok(toml) => (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain")], toml)
            .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

/// Create a new capability.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<CreateCapabilityRequest>,
) -> impl IntoResponse {
    match state.tool_admin.create(req, &tenant.0) {
        Ok(summary) => (StatusCode::CREATED, Json(summary)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Update (replace) a capability's manifest.
pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Json(req): Json<UpdateCapabilityRequest>,
) -> impl IntoResponse {
    match state.tool_admin.update(&name, req, &tenant.0) {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Enable or disable a capability.
pub async fn set_enabled(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Json(payload): Json<SetEnabledPayload>,
) -> impl IntoResponse {
    match state.tool_admin.set_enabled(&name, payload.enabled, &tenant.0) {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

/// Delete a capability permanently.
pub async fn delete_one(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.delete(&name, &tenant.0) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Hot-reload a single capability from disk.
pub async fn reload_one(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.reload(&name, &tenant.0) {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Hot-reload all capabilities from disk.
pub async fn reload_all(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
) -> impl IntoResponse {
    match state.tool_admin.reload_all(&tenant.0) {
        Ok(n) => Json(serde_json::json!({ "reloaded": n })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Validate a manifest TOML without registering it.
pub async fn validate(
    Extension(_tenant): Extension<ResolvedTenant>,
    body: String,
) -> Json<ValidationResponse> {
    let report = RegisteredToolValidator::validate_manifest(&body);
    Json(ValidationResponse::from(report))
}

/// Test-invoke a capability tool.
pub async fn test_invoke(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<TestInvokeRequest>,
) -> impl IntoResponse {
    match state.tool_admin.test_invoke(req, tenant.0.clone()).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
