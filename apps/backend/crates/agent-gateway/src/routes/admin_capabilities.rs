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
use sha2::{Digest, Sha256};
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
    /// Unique reverse-dns-style ID, e.g. "media.time.current-time".
    pub capability_id: String,
    /// Dot-separated namespace, e.g. "media.time".
    pub namespace: String,
    pub version: String,
    /// Must be "remote_mcp" for self-registering MCP services.
    pub kind: String,
    /// MCP server endpoint URL (required when kind = "remote_mcp").
    pub endpoint: Option<String>,
    /// Tool definitions (name + description + JSON Schema).
    pub tools: Vec<ToolDefJson>,
    /// Empty = global. Non-empty = only these tenant IDs.
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
        Ok(toml) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            toml,
        )
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

/// Update (replace) a capability's manifest.
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

/// Enable or disable a capability.
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

/// Hot-reload all capabilities from disk.
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

// ── Dynamic prompt admin ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VersionQuery {
    pub version: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptUpsertRequest {
    pub model: String,
    pub user_template: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<i32>,
    pub vision: Option<bool>,
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct PromptVersionInfo {
    pub capability_name: String,
    pub version: i32,
    pub model: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// `PUT /admin/capabilities/:name/prompt` — create a new prompt version (version = max+1).
pub async fn upsert_prompt(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Json(req): Json<PromptUpsertRequest>,
) -> impl IntoResponse {
    let pool = match &state.pool {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "database not available in test mode",
            )
                .into_response();
        }
    };

    // Read previous latest prompt so we can delta-skip re-embedding when unchanged.
    let prev_latest: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT row_to_json(dp) FROM dynamic_prompts dp
         WHERE capability_name = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    // Compute next version.
    let next_version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM dynamic_prompts WHERE capability_name = $1",
    )
    .bind(&name)
    .fetch_one(&pool)
    .await
    .unwrap_or(1);

    let result = sqlx::query(
        r#"INSERT INTO dynamic_prompts
               (capability_name, version, system_prompt, user_template, model,
                max_tokens, vision, output_schema, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())"#,
    )
    .bind(&name)
    .bind(next_version)
    .bind(&req.system_prompt)
    .bind(&req.user_template)
    .bind(&req.model)
    .bind(req.max_tokens.unwrap_or(1024))
    .bind(req.vision.unwrap_or(false))
    .bind(&req.output_schema)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let model = req.model.clone();
            let user_template = req.user_template.clone();
            let system_prompt = req.system_prompt.clone();
            let max_tokens = req.max_tokens.unwrap_or(1024);
            let vision = req.vision.unwrap_or(false);
            let output_schema = req.output_schema.clone();

            let current = serde_json::json!({
                "system_prompt": system_prompt,
                "user_template": user_template,
                "model": model,
                "max_tokens": max_tokens,
                "vision": vision,
                "output_schema": output_schema,
            });

            let prev_norm = prev_latest.map(|p| {
                serde_json::json!({
                    "system_prompt": p.get("system_prompt").cloned().unwrap_or(serde_json::Value::Null),
                    "user_template": p.get("user_template").cloned().unwrap_or(serde_json::Value::Null),
                    "model": p.get("model").cloned().unwrap_or(serde_json::Value::Null),
                    "max_tokens": p.get("max_tokens").cloned().unwrap_or(serde_json::Value::Null),
                    "vision": p.get("vision").cloned().unwrap_or(serde_json::Value::Null),
                    "output_schema": p.get("output_schema").cloned().unwrap_or(serde_json::Value::Null),
                })
            });

            if prev_norm.as_ref() != Some(&current) {
                let extra = format!(
                    "Prompt model: {}\nSystem: {}\nTemplate: {}",
                    req.model,
                    req.system_prompt.clone().unwrap_or_default(),
                    req.user_template
                );
                if let Some(manifest) = manifest_from_registry(&state, &name)
                    && let Err(e) = sync_manifest_embedding(&state, &manifest, Some(&extra)).await
                {
                    warn!(error = %e, capability = %name, "prompt embedding delta sync failed");
                }
            }

            // Invalidate the semantic router cache for this capability.
            state.semantic_router.invalidate_all().await;
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "capability_name": name, "version": next_version })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /admin/capabilities/:name/prompt` — retrieve a specific or latest prompt version.
pub async fn get_prompt(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    Query(q): Query<VersionQuery>,
) -> impl IntoResponse {
    let pool = match &state.pool {
        Some(p) => p.clone(),
        None => {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };

    let row: Result<serde_json::Value, _> = match q.version {
        Some(v) => {
            sqlx::query_scalar(
                "SELECT row_to_json(dp) FROM dynamic_prompts dp
             WHERE capability_name = $1 AND version = $2",
            )
            .bind(&name)
            .bind(v)
            .fetch_one(&pool)
            .await
        }
        None => {
            sqlx::query_scalar(
                "SELECT row_to_json(dp) FROM dynamic_prompts dp
             WHERE capability_name = $1 ORDER BY version DESC LIMIT 1",
            )
            .bind(&name)
            .fetch_one(&pool)
            .await
        }
    };

    match row {
        Ok(v) => Json(v).into_response(),
        Err(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /admin/capabilities/:name/prompt/versions` — list all prompt versions.
pub async fn list_prompt_versions(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let pool = match &state.pool {
        Some(p) => p.clone(),
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let rows: Result<Vec<serde_json::Value>, _> = sqlx::query_scalar(
        "SELECT row_to_json(t) FROM (
             SELECT capability_name, version, model, updated_at
             FROM dynamic_prompts
             WHERE capability_name = $1
             ORDER BY version DESC
         ) t",
    )
    .bind(&name)
    .fetch_all(&pool)
    .await;

    match rows {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /admin/capabilities/namespaces` — list namespace tree for admin autocomplete.
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

fn manifest_from_registry(
    state: &Arc<AppState>,
    name: &str,
) -> Option<agent_core::capabilities::manifest::ToolManifest> {
    let registry = state.registry.lock().unwrap();
    registry.get(name).map(|c| c.manifest.clone())
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

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    let stored_hash: Option<String> = if let Some(pool) = &state.pool {
        sqlx::query_scalar(
            "SELECT (metadata->>'content_hash')::text FROM capability_embeddings WHERE capability_id = $1",
        )
        .bind(&manifest.name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    if stored_hash.as_deref() == Some(content_hash.as_str()) {
        return Ok(());
    }

    let embedding = state.embedding_service.embed_query(&content).await?;
    let metadata = serde_json::json!({
        "kind": format!("{:?}", manifest.kind),
        "namespace": manifest.namespace(),
        "tags": manifest.tags.clone(),
        "content_hash": content_hash,
    });

    state
        .vector_store
        .upsert_capability_embedding_full(
            &manifest.name,
            &content,
            &embedding,
            &metadata,
            manifest.namespace(),
            &manifest.tags,
        )
        .await?;

    state.semantic_router.invalidate_all().await;
    Ok(())
}

// ── Self-registration endpoint ────────────────────────────────────────────────

/// `POST /admin/capabilities/register` — external services self-register here on startup.
///
/// Idempotent: re-posting the same `(namespace, name)` upserts instead of erroring.
pub async fn register_capability(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CapabilityRegisterRequest>,
) -> Result<impl IntoResponse, HttpError> {
    // ── Service-token auth ────────────────────────────────────────────────────
    // Accepts EITHER:
    //   1. `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>` (static platform token), OR
    //   2. `X-Device-Token: <plaintext>` validated via blake3 hash lookup in the DB.
    // In dev (PLATFORM_ADMIN_TOKEN unset and no X-Device-Token) any call is
    // accepted so zero-config self-registration works out of the box.
    let platform_token = std::env::var("PLATFORM_ADMIN_TOKEN").unwrap_or_default();
    if !platform_token.is_empty() {
        let bearer_ok = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|provided| provided == platform_token)
            .unwrap_or(false);

        if !bearer_ok {
            // Try X-Device-Token fallback.
            let device_token = headers
                .get("x-device-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let device_ok = if !device_token.is_empty() {
                if let Some(pool) = &state.pool {
                    crate::routes::admin_devices::validate_device_token(pool, device_token)
                        .await
                        .unwrap_or(None)
                        .is_some()
                } else {
                    false
                }
            } else {
                false
            };

            if !device_ok {
                return Err(HttpError::auth("invalid or missing PLATFORM_ADMIN_TOKEN"));
            }
        }
    }

    // ── Validate ──────────────────────────────────────────────────────────────
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

    // ── Persist + register each tool individually ─────────────────────────────
    // capability_specs stores one row per tool (namespace + tool_name unique).
    // Each tool gets its own ToolManifest so the invoke path calls the right
    // MCP tool function name.
    for t in &req.tools {
        // Qualified capability name mirrors what row_to_provider produces:
        //   qualified_cap_name(namespace, tool_name)
        let cap_name = if req.namespace.is_empty() {
            t.name.clone()
        } else {
            format!("{}.{}", req.namespace, t.name)
        };

        let tool_def = ToolDef {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
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
            enabled: true,
            search_keywords: vec![],
        };

        // ── Persist to DB ─────────────────────────────────────────────────────
        if let Some(pool) = &state.pool {
            sqlx::query(
                r#"
                INSERT INTO capability_specs
                    (id, namespace, tool_name, description, input_schema, output_schema,
                     strategy, payload, tags, tenant_scope, enabled)
                VALUES
                    (gen_random_uuid(), $1, $2, $3, $4, NULL,
                     'remote_mcp', jsonb_build_object('endpoint', $5::text),
                     $6, $7, $8)
                ON CONFLICT (namespace, tool_name) DO UPDATE SET
                    description  = EXCLUDED.description,
                    payload      = EXCLUDED.payload,
                    tags         = EXCLUDED.tags,
                    tenant_scope = EXCLUDED.tenant_scope,
                    enabled      = EXCLUDED.enabled,
                    updated_at   = now()
                "#,
            )
            .bind(&req.namespace)
            .bind(&t.name) // ← actual tool function name, not service name
            .bind(&t.description)
            .bind(&t.input_schema)
            .bind(endpoint)
            .bind(&req.tags)
            .bind(&req.tenant_scope)
            .bind(req.enabled)
            .execute(pool)
            .await
            .map_err(|e| HttpError::internal(format!("db upsert failed: {e}"), None))?;

            // Embed each tool individually for semantic routing.
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
                        &meta,
                        &req.namespace,
                        &req.tags,
                    )
                    .await;
            }
        }

        // ── Register in-process ───────────────────────────────────────────────
        let card = CapabilityCard::new(manifest.clone(), std::path::PathBuf::from("."));
        let provider = RemoteMcpCapability::new(manifest, endpoint.to_string());
        state
            .registry
            .lock()
            .unwrap()
            .register(card.with_provider(provider));
    }

    // ── Invalidate semantic router cache ──────────────────────────────────────
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

/// Validate a capability_id: starts with lowercase letter, only [a-z0-9._-], max 128 chars.
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
