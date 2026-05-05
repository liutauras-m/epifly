#![allow(clippy::items_after_test_module)]
/// QdrantThreadStore — persists Thread + Message documents in Qdrant.
///
/// Data layout (per tenant, single collection `threads_{tenant_id}`):
///   • Thread doc  — payload.type = "thread",  id = sha256(thread_id) as u64
///   • Message doc — payload.type = "message", id = sha256(thread_id+seq) as u64
///
/// Vectors are 4-dim zeros; Qdrant is used purely as a document store here.
/// Payload filtering (match/must) drives all queries.
use super::qdrant_helpers::{VECTOR_DIM, payload_to_json, point_id};
use async_trait::async_trait;
use chrono::Utc;
use common::limits::{MAX_MESSAGES_BEFORE_SUMMARY, MAX_MESSAGES_PER_THREAD};
use common::memory::store::ThreadStore;
use common::memory::thread::{Message, Thread};
use common::types::ThreadId;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance,
    FieldType, CreateFieldIndexCollectionBuilder, Filter, GetPointsBuilder,
    PointStruct, ScrollPointsBuilder, SetPayloadPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use serde_json::{Value, json};
use std::sync::Arc;
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
        let store = QdrantThreadStore::new("http://localhost:6334");
        assert_eq!(store.collection("acme"), "threads_acme");
        assert_eq!(store.collection("beta"), "threads_beta");
    }
}

pub struct QdrantThreadStore {
    client: Arc<Qdrant>,
    url: String,
}

impl QdrantThreadStore {
    pub fn new(grpc_url: impl Into<String>) -> Self {
        let url = grpc_url.into();
        let client = Arc::new(
            Qdrant::from_url(&url)
                .build()
                .expect("qdrant-client build failed"),
        );
        Self { client, url }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("threads_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        if self.client.collection_exists(&col).await? {
            return Ok(());
        }
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&col)
                    .vectors_config(VectorParamsBuilder::new(VECTOR_DIM as u64, Distance::Cosine)),
            )
            .await?;
        for field in &["type", "thread_id", "tenant_id"] {
            let _ = self
                .client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &col,
                    *field,
                    FieldType::Keyword,
                ))
                .await;
        }
        tracing::info!(collection = col.as_str(), "created Qdrant collection");
        Ok(())
    }

    async fn upsert_point(&self, tenant_id: &str, point: Value) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let pid: u64 = point["id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("missing point id"))?;
        let vector: Vec<f32> = point["vector"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
            .unwrap_or_else(|| vec![0.0_f32; VECTOR_DIM]);
        let payload: qdrant_client::Payload = point["payload"]
            .clone()
            .try_into()
            .map_err(|e| anyhow::anyhow!("payload conversion: {e:?}"))?;
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&col, vec![PointStruct::new(pid, vector, payload)])
                    .wait(true),
            )
            .await?;
        Ok(())
    }

    async fn scroll_filter(
        &self,
        tenant_id: &str,
        filter: Filter,
        limit: usize,
    ) -> anyhow::Result<Vec<Value>> {
        let col = self.collection(tenant_id);
        let resp = self
            .client
            .scroll(
                ScrollPointsBuilder::new(&col)
                    .filter(filter)
                    .limit(limit as u32)
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await?;
        Ok(resp
            .result
            .into_iter()
            .map(|p| json!({ "payload": payload_to_json(p.payload) }))
            .collect())
    }

    fn thread_from_payload(p: &Value) -> Option<Thread> {
        let payload = &p["payload"];
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

    fn message_from_payload(p: &Value) -> Option<Message> {
        let payload = &p["payload"];
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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
                Filter::must([
                    Condition::matches("type", "thread".to_string()),
                    Condition::matches("thread_id", thread_id.to_string()),
                ]),
                1,
            )
            .await?;

        Ok(points.first().and_then(Self::thread_from_payload))
    }

    #[instrument(skip(self), fields(tenant_id, thread_id))]
    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>> {
        self.ensure_collection(tenant_id).await?;

        let mut points = self
            .scroll_filter(
                tenant_id,
                Filter::must([
                    Condition::matches("type", "message".to_string()),
                    Condition::matches("thread_id", thread_id.to_string()),
                ]),
                MAX_MESSAGES_PER_THREAD,
            )
            .await?;

        // Sort by seq ascending (Qdrant scroll is unordered)
        points.sort_by_key(|p| p["payload"]["seq"].as_u64().unwrap_or(0));

        Ok(points
            .iter()
            .filter_map(Self::message_from_payload)
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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
            self.url.clone(),
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
                Filter::must([
                    Condition::matches("type", "thread".to_string()),
                    Condition::matches("tenant_id", tenant_id.to_string()),
                ]),
                // Fetch more when cursor is set so we can trim
                if after.is_some() { limit + 500 } else { limit },
            )
            .await?;

        let mut threads: Vec<Thread> = points
            .iter()
            .filter_map(Self::thread_from_payload)
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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

// Suppress "field `client` is never read" when the gRPC client's methods are
// accessed only through `Arc<Qdrant>` ergonomics.
#[allow(dead_code)]
fn _assert_send_sync()
where
    QdrantThreadStore: Send + Sync,
{
}
