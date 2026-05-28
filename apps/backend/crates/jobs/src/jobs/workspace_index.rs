//! `WorkspaceIndexJob` — durable embedding + vector upsert for workspace content.
//!
//! Enqueued by `patch_content` and `restore_version` instead of a fire-and-forget
//! `tokio::spawn`. The `content_version` (derived from `last_modified` millis at
//! enqueue time) guards against stale upserts: if the node was modified again
//! before this job ran, we skip without error.

use crate::context::JobContext;
use crate::job::BackgroundJob;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{error, info, warn};
use ulid::Ulid;

/// Input payload serialised into the job queue.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceIndexInput {
    pub tenant_id: String,
    pub node_id: String,
    /// `last_modified.timestamp_millis()` captured at enqueue time.
    /// Worker skips if the node has been modified since.
    pub content_version: i64,
}

pub struct WorkspaceIndexJob;

impl WorkspaceIndexJob {
    pub const NAME: &'static str = "workspace-index";
}

#[async_trait]
impl BackgroundJob for WorkspaceIndexJob {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn run(
        &self,
        input: serde_json::Value,
        ctx: Arc<JobContext>,
    ) -> anyhow::Result<serde_json::Value> {
        let payload: WorkspaceIndexInput = serde_json::from_value(input)?;

        let node_id = Ulid::from_string(&payload.node_id)
            .map_err(|e| anyhow::anyhow!("invalid node_id: {e}"))?;

        let workspace_store = ctx
            .workspace_store
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("workspace_store not configured in JobContext"))?;

        let workspace_content = ctx
            .workspace_content
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("workspace_content not configured in JobContext"))?;

        let embedding_svc = ctx
            .embedding_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("embedding_service not configured in JobContext"))?;

        let vector_store = ctx
            .vector_store
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vector_store not configured in JobContext"))?;

        // 1. Fetch the node to check current version and get owner_id / virtual_path.
        let node = match workspace_store
            .get_accessible_node(&payload.tenant_id, "__system__", node_id)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                warn!(
                    tenant_id = %payload.tenant_id,
                    node_id = %node_id,
                    error = %e,
                    "workspace_index: node not found, skipping"
                );
                return Ok(serde_json::json!({ "skipped": "node_not_found" }));
            }
        };

        // 2. Version guard: if the node was modified after this job was enqueued, skip.
        let current_version = node.last_modified.timestamp_millis();
        if current_version > payload.content_version {
            info!(
                workspace_index_skipped_stale = true,
                tenant_id = %payload.tenant_id,
                node_id = %node_id,
                job_version = payload.content_version,
                current_version,
                "workspace_index: stale job skipped"
            );
            return Ok(serde_json::json!({ "skipped": "stale" }));
        }

        // 3. Read current content via stable key when available (Step 3.4 dual-read).
        let (content_key, legacy_key) = match &node.object_key {
            Some(ok) => (ok.as_str(), Some(node.virtual_path.as_str())),
            None => (node.virtual_path.as_str(), None),
        };
        let content = workspace_content
            .read(&payload.tenant_id, content_key, legacy_key)
            .await
            .map_err(|e| anyhow::anyhow!("read content: {e}"))?;

        if content.is_empty() {
            return Ok(serde_json::json!({ "skipped": "empty_content" }));
        }

        // 4. Chunk, embed, upsert.
        let start = std::time::Instant::now();
        const CHUNK: usize = 1500;
        let chunks: Vec<String> = content
            .chars()
            .collect::<Vec<_>>()
            .chunks(CHUNK)
            .map(|c| c.iter().collect::<String>())
            .collect();

        let embeddings = embedding_svc
            .embed_documents(chunks.clone())
            .await
            .map_err(|e| anyhow::anyhow!("embed_documents: {e}"))?;

        let node_id_str = node_id.to_string();
        for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            let chunk_id = format!("{node_id_str}_{current_version}_{i}");
            if let Err(e) = vector_store
                .upsert_content_embedding_full(
                    &chunk_id,
                    &node_id_str,
                    i as i32,
                    chunk,
                    emb,
                    &payload.tenant_id,
                    &node.owner_id,
                    &[],
                )
                .await
            {
                error!(
                    tenant_id = %payload.tenant_id,
                    node_id = %node_id_str,
                    chunk = i,
                    error = %e,
                    workspace_index_failure = true,
                    "workspace_index: upsert failed"
                );
                return Err(anyhow::anyhow!("upsert chunk {i}: {e}"));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            tenant_id = %payload.tenant_id,
            node_id = %node_id_str,
            chunks = chunks.len(),
            duration_ms,
            "workspace_index: indexed"
        );

        Ok(serde_json::json!({
            "indexed_chunks": chunks.len(),
            "duration_ms": duration_ms,
        }))
    }
}
