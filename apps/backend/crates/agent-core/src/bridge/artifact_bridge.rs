//! Post-invoke artifact materialisation bridge.
//!
//! Called after every `CapabilityProvider::invoke()` that returns a `ToolOutput`
//! with non-empty `artifacts`. Uploads binaries to MinIO and inserts `workspace_nodes`
//! rows directly via Postgres.
//!
//! # SRP contract
//! - Tools return `ToolOutput` — they never call object_store directly.
//! - `ArtifactBridge` owns the upload + workspace node creation + index trigger.

use base64::Engine as _;
use common::artifact::{Artifact, ToolOutput};
use object_store::{ObjectStore, path::Path as OsPath};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, instrument, warn};
use ulid::Ulid;

/// MIME types that should be indexed after upload.
const INDEXABLE_MIME_PREFIXES: &[&str] = &["text/", "application/pdf", "application/json"];

pub struct ArtifactBridge {
    pool: PgPool,
    object_store: Arc<dyn ObjectStore>,
}

impl ArtifactBridge {
    pub fn new(pool: PgPool, object_store: Arc<dyn ObjectStore>) -> Arc<Self> {
        Arc::new(Self { pool, object_store })
    }

    /// Materialise all artifacts in `output`. Failures are logged, not propagated.
    #[instrument(skip(self, output), fields(tool = tool_name, artifact_count = output.artifacts.len()))]
    pub async fn process_if_artifacts(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        parent_node_id: Option<&str>,
        output: &ToolOutput,
    ) -> anyhow::Result<()> {
        if output.artifacts.is_empty() {
            return Ok(());
        }
        for artifact in &output.artifacts {
            if let Err(e) = self
                .materialise(tenant_id, user_id, tool_name, parent_node_id, artifact)
                .await
            {
                warn!(error = %e, artifact = %artifact.name, "artifact materialisation failed — skipping");
            }
        }
        Ok(())
    }

    async fn materialise(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        _parent_node_id: Option<&str>,
        artifact: &Artifact,
    ) -> anyhow::Result<()> {
        let node_id = Ulid::new().to_string();
        let object_key = format!("{tenant_id}/{tool_name}/{node_id}/{}", artifact.name);

        // Upload to object store if base64 data is present.
        if let Some(ref b64) = artifact.data {
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
            self.object_store
                .put(&OsPath::from(object_key.as_str()), bytes.into())
                .await?;
        }

        // Create workspace_nodes row (kind='file') directly via Postgres.
        let owner = user_id.unwrap_or("system");
        let virtual_path = format!("/outputs/{tool_name}/{}", artifact.name);
        let metadata = json!({
            "mime_type":          artifact.mime_type,
            "tool":               tool_name,
            "source":             "tool_output",
            "object_key":         object_key.clone(),
            "artifact_metadata":  artifact.metadata,
        });
        sqlx::query(
            r#"
            INSERT INTO workspace_nodes
                (id, tenant_id, owner_id, parent_id, kind, name, virtual_path, metadata)
            VALUES ($1, $2, $3, NULL, 'file', $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&node_id)
        .bind(tenant_id)
        .bind(owner)
        .bind(&artifact.name)
        .bind(&virtual_path)
        .bind(&metadata)
        .execute(&self.pool)
        .await?;

        // Trigger async indexing for text-like MIME types (best-effort).
        if is_indexable(&artifact.mime_type) {
            let pool = self.pool.clone();
            let key = object_key.clone();
            let id = node_id.clone();
            tokio::spawn(async move {
                if let Err(e) = trigger_index(pool, id, key).await {
                    warn!(error = %e, "artifact index trigger failed");
                }
            });
        }

        info!(artifact = %artifact.name, node_id = %node_id, "artifact materialised");
        Ok(())
    }
}

fn is_indexable(mime: &str) -> bool {
    INDEXABLE_MIME_PREFIXES.iter().any(|p| mime.starts_with(p))
}

async fn trigger_index(pool: PgPool, node_id: String, object_key: String) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO indexing_queue (node_id, object_key, created_at)
         VALUES ($1, $2, now())
         ON CONFLICT (node_id) DO NOTHING",
    )
    .bind(&node_id)
    .bind(&object_key)
    .execute(&pool)
    .await?;
    Ok(())
}
