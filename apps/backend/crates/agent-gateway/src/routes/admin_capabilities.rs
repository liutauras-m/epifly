//! Super-admin REST API for managing capabilities at runtime.
//!
//! All routes require `Authorization: Bearer <jwt>` with `role = "super_admin"`.
//! They are protected by the `require_super_admin_jwt` middleware layer.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::capabilities::card::CapabilityCard;
use agent_core::capabilities::manifest::{ToolDef, ToolKind, ToolManifest};
use agent_core::capabilities::providers::remote_mcp::RemoteMcpCapability;
use agent_core::{
    CapabilitySummary, CreateCapabilityRequest, RegisteredToolValidator, TestInvokeRequest,
    UpdateCapabilityRequest, ValidationReport,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

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

/// JSON manifest posted by external self-registering capability services.
#[derive(Debug, Deserialize)]
pub struct CapabilityRegisterRequest {
    pub capability_id: String,
    pub namespace: String,
    pub version: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub tools: Vec<ToolDefJson>,
    #[serde(default)]
    pub tenant_scope: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolDefJson {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub capability_id: String,
    pub registered: bool,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
) -> Json<Vec<CapabilitySummary>> {
    Json(state.tool_admin.list())
}

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

pub async fn get_manifest(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.get_manifest_toml(&name) {
        Ok(toml) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            toml,
        )
            .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<CreateCapabilityRequest>,
) -> impl IntoResponse {
    let manifest =
        agent_core::capabilities::manifest::ToolManifest::from_toml(&req.manifest_toml).ok();
    match state.tool_admin.create(req, &tenant.0) {
        Ok(summary) => {
            if let Some(m) = manifest
                && let Err(e) = sync_manifest_embedding(&state, &m, None).await
            {
                warn!(error = %e, capability = %m.name, "capability embedding sync failed after create");
            }
            (StatusCode::CREATED, Json(summary)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Json(req): Json<UpdateCapabilityRequest>,
) -> impl IntoResponse {
    let manifest =
        agent_core::capabilities::manifest::ToolManifest::from_toml(&req.manifest_toml).ok();
    match state.tool_admin.update(&name, req, &tenant.0) {
        Ok(summary) => {
            if let Some(m) = manifest
                && let Err(e) = sync_manifest_embedding(&state, &m, None).await
            {
                warn!(error = %e, capability = %m.name, "capability embedding sync failed after update");
            }
            Json(summary).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

pub async fn set_enabled(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Json(payload): Json<SetEnabledPayload>,
) -> impl IntoResponse {
    match state
        .tool_admin
        .set_enabled(&name, payload.enabled, &tenant.0)
    {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

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

pub async fn reload_one(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.tool_admin.reload(&name, &tenant.0) {
        Ok(summary) => {
            if let Ok(toml) = state.tool_admin.get_manifest_toml(&name)
                && let Ok(manifest) =
                    agent_core::capabilities::manifest::ToolManifest::from_toml(&toml)
                && let Err(e) = sync_manifest_embedding(&state, &manifest, None).await
            {
                warn!(error = %e, capability = %manifest.name, "capability embedding sync failed after reload");
            }
            Json(summary).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn reload_all(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
) -> impl IntoResponse {
    match state.tool_admin.reload_all(&tenant.0) {
        Ok(n) => {
            for summary in state.tool_admin.list() {
                if let Ok(toml) = state.tool_admin.get_manifest_toml(&summary.name)
                    && let Ok(manifest) =
                        agent_core::capabilities::manifest::ToolManifest::from_toml(&toml)
                    && let Err(e) = sync_manifest_embedding(&state, &manifest, None).await
                {
                    warn!(error = %e, capability = %manifest.name, "capability embedding sync failed after reload_all");
                }
            }
            Json(serde_json::json!({ "reloaded": n })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn validate(
    Extension(_tenant): Extension<ResolvedTenant>,
    body: String,
) -> Json<ValidationResponse> {
    let report = RegisteredToolValidator::validate_manifest(&body);
    Json(ValidationResponse::from(report))
}

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


pub async fn list_namespaces(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let prefix = q.get("prefix").map(|s| s.as_str()).unwrap_or("");
    let registry = state.registry.lock().unwrap();
    let children = registry.namespace_children(prefix);
    Json(serde_json::json!({ "prefix": prefix, "children": children }))
}

async fn sync_manifest_embedding(
    state: &Arc<AppState>,
    manifest: &agent_core::capabilities::manifest::ToolManifest,
    extra_embedding_text: Option<&str>,
) -> anyhow::Result<()> {
    let mut content = manifest.embedding_text();
    if let Some(extra) = extra_embedding_text {
        content.push('\n');
        content.push_str(extra);
    }

    let embedding = state.embedding_service.embed_query(&content).await?;
    let metadata = serde_json::json!({
        "kind": format!("{:?}", manifest.kind),
        "namespace": manifest.namespace(),
        "tags": manifest.tags.clone(),
    });

    state
        .vector_store
        .upsert_capability_embedding_full(
            &manifest.name,
            &content,
            &embedding,
            metadata,
            manifest.namespace(),
            &manifest.tags,
        )
        .await?;

    state.semantic_router.invalidate_all().await;
    Ok(())
}

// ── Self-registration endpoint ────────────────────────────────────────────────

pub async fn register_capability(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CapabilityRegisterRequest>,
) -> Result<impl IntoResponse, HttpError> {
    let platform_token = std::env::var("PLATFORM_ADMIN_TOKEN").unwrap_or_default();
    if !platform_token.is_empty() {
        let bearer_ok = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|provided| provided == platform_token)
            .unwrap_or(false);

        if !bearer_ok {
            let device_token = headers
                .get("x-device-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let device_ok = if !device_token.is_empty() {
                crate::routes::admin_devices::validate_device_token(&state, device_token)
                    .await
                    .unwrap_or(None)
                    .is_some()
            } else {
                false
            };

            if !device_ok {
                return Err(HttpError::auth("invalid or missing PLATFORM_ADMIN_TOKEN"));
            }
        }
    }

    if !is_valid_capability_id(&req.capability_id) {
        return Err(HttpError::validation(
            "capability_id",
            "capability_id must start with [a-z] and contain only [a-z0-9._-] (max 128 chars)",
        ));
    }
    if req.kind != "remote_mcp" {
        return Err(HttpError::validation(
            "kind",
            "only kind=remote_mcp is supported for self-registration",
        ));
    }
    let endpoint = req.endpoint.as_deref().ok_or_else(|| {
        HttpError::validation("endpoint", "endpoint is required for kind=remote_mcp")
    })?;
    if req.tools.is_empty() {
        return Err(HttpError::validation("tools", "tools must be non-empty"));
    }

    for t in &req.tools {
        let cap_name = if req.namespace.is_empty() {
            t.name.clone()
        } else {
            format!("{}.{}", req.namespace, t.name)
        };

        let tool_def = ToolDef {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
            search_keywords: vec![],
            read_before_write: None,
        };
        let manifest = ToolManifest {
            name: cap_name.clone(),
            version: req.version.clone(),
            description: t.description.clone(),
            kind: ToolKind::RemoteMcp,
            tools: vec![tool_def],
            config: serde_json::json!({ "endpoint": endpoint }),
            tags: req.tags.clone(),
            namespace: Some(req.namespace.clone()),
            chain: None,
            tenant_scope: req.tenant_scope.clone(),
            enabled: req.enabled,
            search_keywords: vec![],
            schema_version: "1.0".into(),
            category: None,
            accepts: vec![],
            emits: vec![],
            idempotent: true,
            cost_hint: None,
            requires: vec![],
        };

        // Upsert embedding for semantic routing.
        let embedding_text = manifest.embedding_text();
        if let Ok(emb) = state.embedding_service.embed_query(&embedding_text).await {
            let meta = serde_json::json!({
                "kind": "remote_mcp",
                "namespace": req.namespace,
                "tags": req.tags,
            });
            let embed_id = format!("{}.{}", req.capability_id, t.name);
            let _ = state
                .vector_store
                .upsert_capability_embedding_full(
                    &embed_id,
                    &embedding_text,
                    &emb,
                    meta,
                    &req.namespace,
                    &req.tags,
                )
                .await;
        }

        let card = CapabilityCard::new(manifest.clone(), std::path::PathBuf::from("."));
        let provider = RemoteMcpCapability::new(manifest, endpoint.to_string());
        state
            .registry
            .lock()
            .unwrap()
            .register(card.with_provider(provider));
    }

    state.semantic_router.invalidate_all().await;

    tracing::info!(capability_id = %req.capability_id, endpoint, "capability self-registered");

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            capability_id: req.capability_id,
            registered: true,
        }),
    ))
}

fn is_valid_capability_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-')
}
