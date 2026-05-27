//! Thread persistence and search-index helpers — Step 2.4.
//!
//! Moved from `routes/agent.rs`.

use crate::state::AppState;
use common::memory::thread::Message;
use chrono::Utc;
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

// ── spawn_index_job ───────────────────────────────────────────────────────────

/// Spawn a background task that indexes the 30 most recent thread messages
/// into the workspace node's vector store so conversation history is searchable.
pub fn spawn_index_job(
    state: &Arc<AppState>,
    tenant_id: String,
    node_id: ulid::Ulid,
    thread_id: String,
) {
    let thread_store = Arc::clone(&state.thread_store);
    let emb_svc = Arc::clone(&state.embedding_service);
    let vs = Arc::clone(&state.vector_store);

    tokio::spawn(async move {
        let recent = thread_store
            .messages(&tenant_id, &thread_id)
            .await
            .unwrap_or_default();
        let snippet: String = recent
            .iter()
            .rev()
            .take(30)
            .rev()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let node_id_str = node_id.to_string();
        const CHUNK: usize = 1500;
        let chunks: Vec<String> = snippet
            .chars()
            .collect::<Vec<_>>()
            .chunks(CHUNK)
            .map(|c| c.iter().collect::<String>())
            .collect();

        if let Ok(embeddings) = emb_svc.embed_documents(chunks.clone()).await {
            for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                let chunk_id = format!("{node_id_str}_t{i}");
                let _ = vs
                    .upsert_content_embedding_full(
                        &chunk_id,
                        &node_id_str,
                        i as i32,
                        chunk,
                        emb,
                        &tenant_id,
                        "",
                        &[],
                    )
                    .await;
            }
        }
    });
}
