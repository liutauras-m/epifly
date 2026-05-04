#![allow(clippy::items_after_test_module)]
/// QdrantThreadStore — persists Thread + Message documents in Qdrant.
///
/// Data layout (per tenant, single collection `threads_{tenant_id}`):
///   • Thread doc  — payload.type = "thread",  id = sha256(thread_id) as u64
///   • Message doc — payload.type = "message", id = sha256(thread_id+seq) as u64
///
/// Vectors are 4-dim zeros; Qdrant is used purely as a document store here.
/// Payload filtering (match/must) drives all queries.
use super::qdrant_helpers::{QdrantClient, point_id, zero_vec};
use async_trait::async_trait;
use chrono::Utc;
use common::limits::{MAX_MESSAGES_BEFORE_SUMMARY, MAX_MESSAGES_PER_THREAD};
use common::memory::store::ThreadStore;
use common::memory::thread::{Message, Thread};
use common::types::ThreadId;
use serde_json::{Value, json};
use tracing::{info, instrument, warn};
use ulid::Ulid;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_id_is_deterministic() {
        let a = point_id("thread:01JABC");
        let b = point_id("thread:01JABC");
        assert_eq!(a, b);
    }

    #[test]
    fn point_id_differs_for_different_keys() {
        let a = point_id("thread:01JABC");
        let b = point_id("thread:01JABD");
        assert_ne!(a, b);
    }

    #[test]
    fn collection_name_namespaced_by_tenant() {
        let store = QdrantThreadStore::new("http://localhost:6333");
        assert_eq!(store.collection("acme"), "threads_acme");
        assert_eq!(store.collection("beta"), "threads_beta");
    }
}

pub struct QdrantThreadStore {
    qdrant: QdrantClient,
}

impl QdrantThreadStore {
    pub fn new(qdrant_url: impl Into<String>) -> Self {
        Self {
            qdrant: QdrantClient::new(qdrant_url),
        }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("threads_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        self.qdrant
            .ensure_collection(&col, &["type", "thread_id", "tenant_id"], &[])
            .await
    }

    async fn upsert_point(&self, tenant_id: &str, point: Value) -> anyhow::Result<()> {
        self.qdrant
            .upsert_point(&self.collection(tenant_id), point)
            .await
    }

    async fn scroll_filter(
        &self,
        tenant_id: &str,
        filter: Value,
        limit: usize,
    ) -> anyhow::Result<Vec<Value>> {
        self.qdrant
            .scroll_filter(&self.collection(tenant_id), filter, limit)
            .await
    }

    fn thread_from_payload(payload: &Value) -> Option<Thread> {
        Some(Thread {
            id: payload["thread_id"].as_str()?.parse::<ThreadId>().ok()?,
            tenant_id: payload["tenant_id"].as_str()?.to_owned(),
            title: payload["title"].as_str().map(String::from),
            created_at: payload["created_at"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            last_active: payload["last_active"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            message_count: payload["message_count"].as_u64().unwrap_or(0) as usize,
            summary: payload["summary"].as_str().map(String::from),
            metadata: payload["metadata"].clone(),
        })
    }

    fn message_from_payload(payload: &Value) -> Option<Message> {
        Some(Message {
            role: payload["role"].as_str()?.to_owned(),
            content: payload["content"].as_str()?.to_owned(),
            tool_calls: None,
            timestamp: payload["timestamp"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            seq: payload["seq"].as_u64().unwrap_or(0) as usize,
        })
    }

    /// Spawn background summarisation when message count crosses threshold.
    fn maybe_summarise(
        &self,
        tenant_id: String,
        thread_id: String,
        message_count: usize,
        qdrant_url: String,
    ) {
        if message_count < MAX_MESSAGES_BEFORE_SUMMARY
            || !message_count.is_multiple_of(MAX_MESSAGES_BEFORE_SUMMARY)
        {
            return;
        }

        tokio::spawn(async move {
            let store = QdrantThreadStore::new(&qdrant_url);
            match summarise_thread(&store, &tenant_id, &thread_id).await {
                Ok(summary) => {
                    if let Err(e) = store.set_summary(&tenant_id, &thread_id, summary).await {
                        warn!(error = %e, thread_id, "failed to persist thread summary");
                    }
                }
                Err(e) => warn!(error = %e, thread_id, "thread summarisation failed"),
            }
        });
    }
}

/// Call Claude to summarise recent messages, returns a short summary string.
async fn summarise_thread(
    store: &QdrantThreadStore,
    tenant_id: &str,
    thread_id: &str,
) -> anyhow::Result<String> {
    let messages = store.messages(tenant_id, thread_id).await?;
    if messages.is_empty() {
        return Ok(String::new());
    }

    let history: String = messages
        .iter()
        .take(MAX_MESSAGES_BEFORE_SUMMARY)
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let body = json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 512,
        "messages": [{
            "role": "user",
            "content": format!(
                "Summarise the following conversation in 2-3 sentences for use as context:\n\n{history}"
            )
        }]
    });

    let client = reqwest::Client::new();
    let res: Value = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    let summary = res["content"][0]["text"].as_str().unwrap_or("").to_string();

    info!(thread_id, chars = summary.len(), "thread summarised");
    Ok(summary)
}

#[async_trait]
impl ThreadStore for QdrantThreadStore {
    #[instrument(skip(self, initial_messages), fields(tenant_id, thread_id = tracing::field::Empty))]
    async fn create(
        &self,
        tenant_id: &str,
        initial_messages: Vec<Message>,
    ) -> anyhow::Result<Thread> {
        self.ensure_collection(tenant_id).await?;

        let thread_id = Ulid::new().to_string();
        tracing::Span::current().record("thread_id", thread_id.as_str());
        let now = Utc::now();

        let thread = Thread {
            id: thread_id.parse::<ThreadId>().expect("just-generated ULID is always valid"),
            tenant_id: tenant_id.to_owned(),
            title: None,
            created_at: now,
            last_active: now,
            message_count: 0,
            summary: None,
            metadata: json!({}),
        };

        let point = json!({
            "id": point_id(&thread_id),
            "vector": zero_vec(),
            "payload": {
                "type": "thread",
                "thread_id": thread_id,
                "tenant_id": tenant_id,
                "title": null,
                "created_at": now.to_rfc3339(),
                "last_active": now.to_rfc3339(),
                "message_count": 0u64,
                "summary": null,
                "metadata": {}
            }
        });
        self.upsert_point(tenant_id, point).await?;

        for msg in initial_messages {
            self.append(tenant_id, &thread_id, msg).await?;
        }

        // Re-fetch to get accurate message_count after appends
        Ok(self.get(tenant_id, &thread_id).await?.unwrap_or(thread))
    }

    #[instrument(skip(self), fields(tenant_id, thread_id))]
    async fn get(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Option<Thread>> {
        self.ensure_collection(tenant_id).await?;

        let points = self
            .scroll_filter(
                tenant_id,
                json!({
                    "must": [
                        {"key": "type",      "match": {"value": "thread"}},
                        {"key": "thread_id", "match": {"value": thread_id}}
                    ]
                }),
                1,
            )
            .await?;

        Ok(points
            .first()
            .and_then(|p| Self::thread_from_payload(&p["payload"])))
    }

    #[instrument(skip(self), fields(tenant_id, thread_id))]
    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>> {
        self.ensure_collection(tenant_id).await?;

        let mut points = self
            .scroll_filter(
                tenant_id,
                json!({
                    "must": [
                        {"key": "type",      "match": {"value": "message"}},
                        {"key": "thread_id", "match": {"value": thread_id}}
                    ]
                }),
                MAX_MESSAGES_PER_THREAD,
            )
            .await?;

        // Sort by seq ascending (Qdrant scroll is unordered)
        points.sort_by_key(|p| p["payload"]["seq"].as_u64().unwrap_or(0));

        Ok(points
            .iter()
            .filter_map(|p| Self::message_from_payload(&p["payload"]))
            .collect())
    }

    #[instrument(skip(self, message), fields(tenant_id, thread_id, role = message.role.as_str()))]
    async fn append(
        &self,
        tenant_id: &str,
        thread_id: &str,
        message: Message,
    ) -> anyhow::Result<()> {
        self.ensure_collection(tenant_id).await?;

        // Read current count to derive seq
        let current = self.get(tenant_id, thread_id).await?;
        let seq = current.as_ref().map(|t| t.message_count).unwrap_or(0);
        let new_count = seq + 1;
        let now = Utc::now();

        // Upsert message point
        let msg_key = format!("{thread_id}:msg:{seq}");
        let msg_point = json!({
            "id": point_id(&msg_key),
            "vector": zero_vec(),
            "payload": {
                "type": "message",
                "thread_id": thread_id,
                "tenant_id": tenant_id,
                "role": message.role,
                "content": message.content,
                "timestamp": message.timestamp.to_rfc3339(),
                "seq": seq as u64
            }
        });
        self.upsert_point(tenant_id, msg_point).await?;

        // Update thread metadata
        let thread_point = json!({
            "id": point_id(thread_id),
            "vector": zero_vec(),
            "payload": {
                "type": "thread",
                "thread_id": thread_id,
                "tenant_id": tenant_id,
                "title": current.as_ref().and_then(|t| t.title.as_deref()),
                "created_at": current.as_ref()
                    .map(|t| t.created_at.to_rfc3339())
                    .unwrap_or_else(|| now.to_rfc3339()),
                "last_active": now.to_rfc3339(),
                "message_count": new_count as u64,
                "summary": current.as_ref().and_then(|t| t.summary.as_deref()),
                "metadata": current.as_ref().map(|t| t.metadata.clone()).unwrap_or(json!({}))
            }
        });
        self.upsert_point(tenant_id, thread_point).await?;

        self.maybe_summarise(
            tenant_id.to_owned(),
            thread_id.to_owned(),
            new_count,
            self.qdrant.base_url.clone(),
        );

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>> {
        self.ensure_collection(tenant_id).await?;

        let points = self
            .scroll_filter(
                tenant_id,
                json!({
                    "must": [
                        {"key": "type",      "match": {"value": "thread"}},
                        {"key": "tenant_id", "match": {"value": tenant_id}}
                    ]
                }),
                // Fetch more when cursor is set so we can trim
                if after.is_some() { limit + 500 } else { limit },
            )
            .await?;

        let mut threads: Vec<Thread> = points
            .iter()
            .filter_map(|p| Self::thread_from_payload(&p["payload"]))
            .collect();

        // Newest first
        threads.sort_by_key(|t| std::cmp::Reverse(t.last_active));

        // Apply cursor
        if let Some(cursor) = after
            && let Some(pos) = threads.iter().position(|t| t.id.to_string() == cursor)
        {
            threads = threads.into_iter().skip(pos + 1).collect();
        }

        threads.truncate(limit);
        Ok(threads)
    }

    #[instrument(skip(self, summary), fields(tenant_id, thread_id))]
    async fn set_summary(
        &self,
        tenant_id: &str,
        thread_id: &str,
        summary: String,
    ) -> anyhow::Result<()> {
        let current = self.get(tenant_id, thread_id).await?;
        let Some(t) = current else { return Ok(()) };
        let now = Utc::now();

        let point = json!({
            "id": point_id(thread_id),
            "vector": zero_vec(),
            "payload": {
                "type": "thread",
                "thread_id": thread_id,
                "tenant_id": tenant_id,
                "title": t.title,
                "created_at": t.created_at.to_rfc3339(),
                "last_active": now.to_rfc3339(),
                "message_count": t.message_count as u64,
                "summary": summary,
                "metadata": t.metadata
            }
        });
        self.upsert_point(tenant_id, point).await
    }

    #[instrument(skip(self, title), fields(tenant_id, thread_id))]
    async fn set_title(
        &self,
        tenant_id: &str,
        thread_id: &str,
        title: String,
    ) -> anyhow::Result<()> {
        let current = self.get(tenant_id, thread_id).await?;
        let Some(t) = current else { return Ok(()) };

        let point = json!({
            "id": point_id(thread_id),
            "vector": zero_vec(),
            "payload": {
                "type": "thread",
                "thread_id": thread_id,
                "tenant_id": tenant_id,
                "title": title,
                "created_at": t.created_at.to_rfc3339(),
                "last_active": t.last_active.to_rfc3339(),
                "message_count": t.message_count as u64,
                "summary": t.summary,
                "metadata": t.metadata
            }
        });
        self.upsert_point(tenant_id, point).await
    }
}
