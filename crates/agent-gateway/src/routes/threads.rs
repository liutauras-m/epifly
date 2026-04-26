/// Thread (persistent conversation memory) REST API.
///
/// POST   /v1/threads                          → create thread
/// GET    /v1/threads                          → list threads (newest first)
/// GET    /v1/threads/{thread_id}              → get thread metadata
/// GET    /v1/threads/{thread_id}/messages     → get messages (ordered)
/// POST   /v1/threads/{thread_id}/messages     → append a message
/// DELETE /v1/threads/{thread_id}              → (future: not yet implemented)
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::Utc;
use common::memory::thread::{Message, Thread};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::instrument;

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateThreadRequest {
    pub messages: Option<Vec<MessageInput>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct MessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

fn thread_to_json(t: &Thread) -> Value {
    json!({
        "id": t.id,
        "object": "thread",
        "tenant_id": t.tenant_id,
        "title": t.title,
        "created_at": t.created_at.timestamp(),
        "last_active": t.last_active.timestamp(),
        "message_count": t.message_count,
        "has_summary": t.summary.is_some(),
        "metadata": t.metadata,
    })
}

fn message_to_json(m: &Message) -> Value {
    json!({
        "object": "thread.message",
        "role": m.role,
        "content": m.content,
        "timestamp": m.timestamp.timestamp(),
        "seq": m.seq,
    })
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /v1/threads
#[instrument(skip(state, tenant, req), fields(tenant_id = tenant.0.tenant_id.as_str()))]
pub async fn create_thread(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<CreateThreadRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let initial_messages: Vec<Message> = req
        .messages
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(seq, m)| Message {
            role: m.role,
            content: m.content,
            tool_calls: None,
            timestamp: Utc::now(),
            seq,
        })
        .collect();

    let thread = state
        .thread_store
        .create(&tenant.0.tenant_id, initial_messages)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?;

    Ok(Json(thread_to_json(&thread)))
}

/// GET /v1/threads
#[instrument(skip(state, tenant), fields(tenant_id = tenant.0.tenant_id.as_str()))]
pub async fn list_threads(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let threads = state
        .thread_store
        .list(&tenant.0.tenant_id, q.limit)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?;

    let data: Vec<Value> = threads.iter().map(thread_to_json).collect();
    Ok(Json(json!({"object": "list", "data": data})))
}

/// GET /v1/threads/{thread_id}
#[instrument(skip(state, tenant), fields(tenant_id = tenant.0.tenant_id.as_str()))]
pub async fn get_thread(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let thread = state
        .thread_store
        .get(&tenant.0.tenant_id, &thread_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": {"message": "thread not found"}})),
            )
        })?;

    Ok(Json(thread_to_json(&thread)))
}

/// GET /v1/threads/{thread_id}/messages
#[instrument(skip(state, tenant), fields(tenant_id = tenant.0.tenant_id.as_str()))]
pub async fn get_messages(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let messages = state
        .thread_store
        .messages(&tenant.0.tenant_id, &thread_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?;

    let data: Vec<Value> = messages.iter().map(message_to_json).collect();
    Ok(Json(json!({"object": "list", "data": data})))
}

/// POST /v1/threads/{thread_id}/messages
#[instrument(skip(state, tenant, req), fields(tenant_id = tenant.0.tenant_id.as_str()))]
pub async fn append_message(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
    Json(req): Json<MessageInput>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Verify thread exists
    state
        .thread_store
        .get(&tenant.0.tenant_id, &thread_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": {"message": "thread not found"}})),
            )
        })?;

    let message = Message {
        role: req.role,
        content: req.content,
        tool_calls: None,
        timestamp: Utc::now(),
        seq: 0,
    };

    state
        .thread_store
        .append(&tenant.0.tenant_id, &thread_id, message.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": {"message": e.to_string()}})),
            )
        })?;

    Ok(Json(message_to_json(&message)))
}
