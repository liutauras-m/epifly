//! Super-admin REST API for managing capabilities at runtime.
//!
//! The admin routes require a bearer token with the Zitadel `super_admin` role
//! (`auth::require_super_admin`). `register_capability` is the exception: it is
//! authenticated with `PLATFORM_ADMIN_TOKEN` for service-to-service registration.

use crate::auth::ResolvedTenant;
use crate::state::AppState;
use agent_core::capabilities::card::CapabilityCard;
use agent_core::capabilities::manifest::{ToolDef, ToolKind, ToolManifest};
use agent_core::capabilities::mcp_allowlist::McpAllowlist;
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
#[serde(deny_unknown_fields)]
pub struct CapabilityRegisterRequest {
    pub capability_id: String,
    pub namespace: String,
    pub version: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub tools: Vec<ToolDefJson>,
    /// Tenants that may see the capability. Required unless `global` is true.
    #[serde(default)]
    pub tenant_scope: Vec<String>,
    /// Explicit opt-in to make the capability visible to every tenant.
    #[serde(default)]
    pub global: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

const MAX_REGISTERED_TOOLS: usize = 32;
const MAX_DESCRIPTION_LEN: usize = 2_000;
const MAX_TAGS: usize = 16;

fn is_valid_tool_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Validate a registration and return the capability names it would create.
fn validate_registration(req: &CapabilityRegisterRequest) -> Result<Vec<String>, HttpError> {
    if !is_valid_capability_id(&req.capability_id) {
        return Err(HttpError::validation(
            "capability_id",
            "capability_id must start with [a-z] and contain only [a-z0-9._-] (max 128 chars)",
        ));
    }
    if req.namespace.is_empty() || !RegisteredToolValidator::validate_namespace(&req.namespace).ok()
    {
        return Err(HttpError::validation(
            "namespace",
            "namespace must be dot-separated [a-z][a-z0-9_]* segments (max 6)",
        ));
    }
    if req.version.trim().is_empty() || req.version.len() > 64 {
        return Err(HttpError::validation(
            "version",
            "version is required (max 64 chars)",
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
    McpAllowlist::from_env()
        .check(endpoint)
        .map_err(|e| HttpError::validation("endpoint", e))?;
    if req.tools.is_empty() || req.tools.len() > MAX_REGISTERED_TOOLS {
        return Err(HttpError::validation(
            "tools",
            format!("tools must contain 1..={MAX_REGISTERED_TOOLS} entries"),
        ));
    }
    match (req.global, req.tenant_scope.is_empty()) {
        (false, true) => {
            return Err(HttpError::validation(
                "tenant_scope",
                "tenant_scope is required; set global=true to expose the capability to every tenant",
            ));
        }
        (true, false) => {
            return Err(HttpError::validation(
                "tenant_scope",
                "a global capability must not list tenant_scope",
            ));
        }
        _ => {}
    }
    if req
        .tenant_scope
        .iter()
        .any(|t| t.trim().is_empty() || t.len() > 128)
    {
        return Err(HttpError::validation("tenant_scope", "invalid tenant id"));
    }
    if req.tags.len() > MAX_TAGS || req.tags.iter().any(|t| t.is_empty() || t.len() > 64) {
        return Err(HttpError::validation(
            "tags",
            format!("at most {MAX_TAGS} tags of 1..=64 chars"),
        ));
    }
    let mut names = Vec::with_capacity(req.tools.len());
    for t in &req.tools {
        if !is_valid_tool_name(&t.name) {
            return Err(HttpError::validation(
                "tools.name",
                format!("invalid tool name {:?}", t.name),
            ));
        }
        if t.description.trim().is_empty() || t.description.len() > MAX_DESCRIPTION_LEN {
            return Err(HttpError::validation(
                "tools.description",
                format!("description is required (max {MAX_DESCRIPTION_LEN} chars)"),
            ));
        }
        if !t.input_schema.is_object() {
            return Err(HttpError::validation(
                "tools.input_schema",
                "input_schema must be a JSON Schema object",
            ));
        }
        let name = format!("{}.{}", req.namespace, t.name);
        if names.contains(&name) {
            return Err(HttpError::validation(
                "tools.name",
                format!("duplicate tool {:?}", t.name),
            ));
        }
        names.push(name);
    }
    Ok(names)
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

/// `POST /admin/capabilities/register` — external MCP services register themselves.
///
/// Requires `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>` (constant-time compare).
/// The manifest is validated, the endpoint must pass the exact-host MCP allowlist,
/// the capability is tenant-scoped unless `global` is set, and an existing
/// capability name is never replaced (409).
pub async fn register_capability(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CapabilityRegisterRequest>,
) -> Result<impl IntoResponse, HttpError> {
    if !crate::auth::has_platform_token(&state, &headers) {
        return Err(HttpError::auth("invalid or missing platform token"));
    }
    let names = validate_registration(&req)?;
    let endpoint = req.endpoint.clone().unwrap_or_default();

    let mut providers = Vec::with_capacity(req.tools.len());
    for (t, cap_name) in req.tools.iter().zip(&names) {
        let manifest = ToolManifest {
            name: cap_name.clone(),
            version: req.version.clone(),
            description: t.description.clone(),
            kind: ToolKind::RemoteMcp,
            tools: vec![ToolDef {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
                search_keywords: vec![],
                read_before_write: None,
            }],
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
        let provider = RemoteMcpCapability::new(manifest.clone(), endpoint.clone())
            .map_err(|e| HttpError::validation("endpoint", e.to_string()))?;
        providers.push((manifest, provider));
    }

    // Check-and-insert under one lock: never overwrite an existing capability.
    {
        let mut registry = state.registry.lock().unwrap();
        if let Some(taken) = names.iter().find(|n| registry.get(n).is_some()) {
            return Err(HttpError::conflict(format!(
                "capability {taken} already exists"
            )));
        }
        for (manifest, provider) in &providers {
            let card = CapabilityCard::new(manifest.clone(), std::path::PathBuf::from("."));
            registry.register(card.with_provider(provider.clone()));
        }
    }

    for (manifest, _) in &providers {
        if let Err(e) = sync_manifest_embedding(&state, manifest, None).await {
            warn!(error = %e, capability = %manifest.name, "capability embedding sync failed after registration");
        }
    }
    state.semantic_router.invalidate_all().await;

    tracing::info!(capability_id = %req.capability_id, endpoint, tools = names.len(), "capability self-registered");

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
