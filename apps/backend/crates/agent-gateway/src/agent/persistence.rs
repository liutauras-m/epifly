//! Thread persistence and search-index helpers — Step 2.4.
//!
//! Moved from `routes/agent.rs`.

use crate::state::AppState;
use chrono::Utc;
use common::memory::thread::Message;
use std::sync::Arc;

// ── maybe_set_title ───────────────────────────────────────────────────────────

/// Set an auto-generated title for the thread if one is not yet set.
/// Returns `true` if `set_title` was actually invoked (so the caller can emit
/// a `threads` invalidation event). PR 3.A.6.
pub async fn maybe_set_title(
    store: &Arc<dyn common::memory::ThreadStore>,
    tenant_id: &str,
    thread_id: &str,
    assistant_text: &str,
) -> bool {
    let already_titled = store
        .get(tenant_id, thread_id)
        .await
        .ok()
        .flatten()
        .map(|t| t.title.is_some())
        .unwrap_or(false);

    if !already_titled && !assistant_text.is_empty() {
        let title: String = assistant_text.chars().take(60).collect();
        return store.set_title(tenant_id, thread_id, title).await.is_ok();
    }
    false
}

// ── persist_assistant_message ─────────────────────────────────────────────────

/// Append the final assistant turn text to the thread store.
pub async fn persist_assistant_message(
    state: &Arc<AppState>,
    tenant_id: &str,
    thread_id: &str,
    text: &str,
) {
    let _ = state
        .thread_store
        .append(
            tenant_id,
            thread_id,
            Message {
                role: "assistant".into(),
                content: text.to_string(),
                tool_calls: None,
                timestamp: Utc::now(),
                seq: 0,
            },
        )
        .await;
}

// ── enqueue_projection_job ────────────────────────────────────────────────────

/// Enqueue a durable `ThreadProjectionJob` for `thread_id`.
///
/// Replaces the old fire-and-forget `spawn_index_job`. If a job is already
/// running for this `(tenant_id, thread_id)`, the coalescer bumps a dirty flag
/// instead of spawning a second job.
pub fn enqueue_projection_job(
    state: &Arc<AppState>,
    tenant_id: String,
    thread_id: String,
    node_id: ulid::Ulid,
) {
    use jobs::jobs::{ProjectionReason, ThreadProjectionInput};

    // Try to claim a slot. If None, a job is already running (dirty bumped).
    let Some(_dirty) = state.projection_coalescer.try_claim(&tenant_id, &thread_id) else {
        return;
    };

    let folder = format!("Conversations/{node_id}");
    let input = serde_json::to_value(ThreadProjectionInput {
        tenant_id: tenant_id.clone(),
        thread_id: thread_id.clone(),
        reason: ProjectionReason::AssistantDone,
        folder_path: Some(folder),
    })
    .expect("ThreadProjectionInput is serializable");

    let executor = Arc::clone(&state.job_executor);
    let coalescer = Arc::clone(&state.projection_coalescer);
    let tenant = tenant_id.clone();
    let thread = thread_id.clone();

    tokio::spawn(async move {
        if let Err(e) = executor
            .enqueue(jobs::jobs::ThreadProjectionJob::NAME, input)
            .await
        {
            tracing::warn!(
                tenant_id = %tenant,
                thread_id = %thread,
                error = %e,
                "failed to enqueue ThreadProjectionJob"
            );
            // Release the coalescer slot so future turns can try again.
            coalescer.release(&tenant, &thread);
        }
        // On success, the job itself calls coalescer.release() when it finishes.
    });
}

/// Retained for backward compatibility — callers outside the projection path.
#[deprecated(note = "use enqueue_projection_job for new code")]
pub fn spawn_index_job(
    state: &Arc<AppState>,
    tenant_id: String,
    node_id: ulid::Ulid,
    thread_id: String,
) {
    enqueue_projection_job(state, tenant_id, thread_id, node_id);
}
