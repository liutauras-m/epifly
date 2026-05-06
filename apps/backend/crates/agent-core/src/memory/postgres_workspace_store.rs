use async_trait::async_trait;
use chrono::Utc;
use common::error::ConusAiError;
use common::memory::store::WorkspaceStore;
use common::memory::workspace::{NodeKind, WorkspaceNode, join_virtual_path, validate_name};
use common::metrics;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;
use tracing::{instrument, warn};
use ulid::Ulid;

use crate::indexing::EmbeddingService;
use crate::vector_store::PgVectorStore;

pub struct PostgresWorkspaceStore {
    pool: PgPool,
    embedding_svc: Arc<dyn EmbeddingService>,
    vector_store: Arc<PgVectorStore>,
}

impl PostgresWorkspaceStore {
    pub fn new(
        pool: PgPool,
        embedding_svc: Arc<dyn EmbeddingService>,
        vector_store: Arc<PgVectorStore>,
    ) -> Self {
        Self {
            pool,
            embedding_svc,
            vector_store,
        }
    }

    fn parse_kind(s: &str) -> NodeKind {
        match s {
            "folder" => NodeKind::Folder,
            "conversation" => NodeKind::Conversation,
            _ => NodeKind::File,
        }
    }
}

// ── shared row-to-node mapper ─────────────────────────────────────────────────

struct NodeRow {
    id: String,
    tenant_id: String,
    owner_id: String,
    parent_id: Option<String>,
    kind: String,
    name: String,
    virtual_path: String,
    last_modified: chrono::DateTime<Utc>,
    shared_with: Vec<String>,
    metadata: serde_json::Value,
}

impl From<NodeRow> for WorkspaceNode {
    fn from(r: NodeRow) -> Self {
        WorkspaceNode {
            id: r.id.parse::<Ulid>().unwrap_or_else(|_| Ulid::new()),
            tenant_id: r.tenant_id,
            owner_id: r.owner_id,
            parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
            kind: PostgresWorkspaceStore::parse_kind(&r.kind),
            name: r.name,
            virtual_path: r.virtual_path,
            last_modified: r.last_modified,
            shared_with: r.shared_with,
            metadata: r.metadata,
        }
    }
}

#[async_trait]
impl WorkspaceStore for PostgresWorkspaceStore {
    #[instrument(skip(self), fields(tenant_id, owner_id))]
    async fn create_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Folder)?;

        let parent_path = if let Some(pid) = parent_id {
            let row = sqlx::query_scalar!(
                "SELECT virtual_path FROM workspace_nodes WHERE id = $1 AND tenant_id = $2",
                pid.to_string(),
                tenant_id,
            )
            .fetch_optional(&self.pool)
            .await?;
            row.unwrap_or_default()
        } else {
            String::new()
        };

        let virtual_path = join_virtual_path(
            if parent_path.is_empty() {
                None
            } else {
                Some(parent_path.as_str())
            },
            name,
        );
        let id = Ulid::new().to_string();
        let now = Utc::now();

        sqlx::query!(
            "INSERT INTO workspace_nodes
                (id, tenant_id, owner_id, parent_id, kind, name, virtual_path, last_modified, shared_with, metadata)
             VALUES ($1, $2, $3, $4, 'folder', $5, $6, $7, '{}', '{}'::jsonb)",
            id,
            tenant_id,
            owner_id,
            parent_id.map(|p| p.to_string()),
            name,
            virtual_path,
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(WorkspaceNode {
            id: id.parse().unwrap(),
            tenant_id: tenant_id.to_owned(),
            owner_id: owner_id.to_owned(),
            parent_id,
            kind: NodeKind::Folder,
            name: name.to_owned(),
            virtual_path,
            last_modified: now,
            shared_with: vec![],
            metadata: serde_json::Value::Null,
        })
    }

    #[instrument(skip(self), fields(tenant_id, owner_id))]
    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Conversation)?;

        let parent_path = if let Some(pid) = parent_id {
            let row = sqlx::query_scalar!(
                "SELECT virtual_path FROM workspace_nodes WHERE id = $1 AND tenant_id = $2",
                pid.to_string(),
                tenant_id,
            )
            .fetch_optional(&self.pool)
            .await?;
            row.unwrap_or_default()
        } else {
            String::new()
        };

        let virtual_path = join_virtual_path(
            if parent_path.is_empty() {
                None
            } else {
                Some(parent_path.as_str())
            },
            name,
        );
        let id = Ulid::new().to_string();
        let now = Utc::now();

        sqlx::query!(
            "INSERT INTO workspace_nodes
                (id, tenant_id, owner_id, parent_id, kind, name, virtual_path, last_modified, shared_with, metadata)
             VALUES ($1, $2, $3, $4, 'conversation', $5, $6, $7, '{}', '{}'::jsonb)",
            id,
            tenant_id,
            owner_id,
            parent_id.map(|p| p.to_string()),
            name,
            virtual_path,
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(WorkspaceNode {
            id: id.parse().unwrap(),
            tenant_id: tenant_id.to_owned(),
            owner_id: owner_id.to_owned(),
            parent_id,
            kind: NodeKind::Conversation,
            name: name.to_owned(),
            virtual_path,
            last_modified: now,
            shared_with: vec![],
            metadata: serde_json::Value::Null,
        })
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent_id: Option<Ulid>,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let t_start = Instant::now();
        let nodes: Vec<WorkspaceNode> = if let Some(pid) = parent_id {
            sqlx::query!(
                "SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                        last_modified, shared_with, metadata
                 FROM workspace_nodes
                 WHERE tenant_id = $1 AND parent_id = $2
                   AND (owner_id = $3 OR $3 = ANY(shared_with))
                 ORDER BY kind, name",
                tenant_id,
                pid.to_string(),
                user_id,
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| WorkspaceNode {
                id: r.id.parse().unwrap_or_else(|_| Ulid::new()),
                tenant_id: r.tenant_id,
                owner_id: r.owner_id,
                parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(&r.kind),
                name: r.name,
                virtual_path: r.virtual_path,
                last_modified: r.last_modified,
                shared_with: r.shared_with,
                metadata: r.metadata,
            })
            .collect()
        } else {
            sqlx::query!(
                "SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                        last_modified, shared_with, metadata
                 FROM workspace_nodes
                 WHERE tenant_id = $1 AND parent_id IS NULL
                   AND (owner_id = $2 OR $2 = ANY(shared_with))
                 ORDER BY kind, name",
                tenant_id,
                user_id,
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| WorkspaceNode {
                id: r.id.parse().unwrap_or_else(|_| Ulid::new()),
                tenant_id: r.tenant_id,
                owner_id: r.owner_id,
                parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(&r.kind),
                name: r.name,
                virtual_path: r.virtual_path,
                last_modified: r.last_modified,
                shared_with: r.shared_with,
                metadata: r.metadata,
            })
            .collect()
        };

        let elapsed = t_start.elapsed().as_secs_f64() * 1000.0;
        metrics::db_query_duration_ms().record(elapsed, &[]);

        Ok(nodes)
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        id: Ulid,
    ) -> anyhow::Result<WorkspaceNode> {
        let row = sqlx::query!(
            "SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                    last_modified, shared_with, metadata
             FROM workspace_nodes
             WHERE id = $1 AND tenant_id = $2 AND (owner_id = $3 OR $3 = ANY(shared_with))",
            id.to_string(),
            tenant_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(WorkspaceNode {
                id: r.id.parse().unwrap_or_else(|_| Ulid::new()),
                tenant_id: r.tenant_id,
                owner_id: r.owner_id,
                parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(&r.kind),
                name: r.name,
                virtual_path: r.virtual_path,
                last_modified: r.last_modified,
                shared_with: r.shared_with,
                metadata: r.metadata,
            }),
            None => Err(anyhow::anyhow!(ConusAiError::NotFound(format!(
                "node {id}"
            )))),
        }
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let rows = sqlx::query!(
            r#"WITH RECURSIVE ancestors AS (
               SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                      last_modified, shared_with, metadata
               FROM workspace_nodes WHERE id = $1 AND tenant_id = $2
               UNION ALL
               SELECT n.id, n.tenant_id, n.owner_id, n.parent_id, n.kind, n.name, n.virtual_path,
                      n.last_modified, n.shared_with, n.metadata
               FROM workspace_nodes n
               JOIN ancestors a ON n.id = a.parent_id
            )
            SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                   last_modified, shared_with, metadata
            FROM ancestors
            WHERE owner_id = $3 OR $3 = ANY(shared_with)
            ORDER BY virtual_path"#,
            node_id.to_string(),
            tenant_id,
            user_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| WorkspaceNode {
                id: r
                    .id
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(Ulid::new),
                tenant_id: r.tenant_id.unwrap_or_default(),
                owner_id: r.owner_id.unwrap_or_default(),
                parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(r.kind.as_deref().unwrap_or("folder")),
                name: r.name.unwrap_or_default(),
                virtual_path: r.virtual_path.unwrap_or_default(),
                last_modified: r.last_modified.unwrap_or_else(Utc::now),
                shared_with: r.shared_with.unwrap_or_default(),
                metadata: r.metadata.unwrap_or(serde_json::Value::Null),
            })
            .collect())
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        new_parent_path: Option<&str>,
    ) -> anyhow::Result<WorkspaceNode> {
        // Verify ownership
        let node = self
            .get_accessible_node(tenant_id, user_id, node_id)
            .await?;

        let new_parent_path_str = match new_parent_path {
            Some(p) => p.to_owned(),
            None => {
                if let Some(pid) = new_parent {
                    sqlx::query_scalar!(
                        "SELECT virtual_path FROM workspace_nodes WHERE id = $1",
                        pid.to_string(),
                    )
                    .fetch_optional(&self.pool)
                    .await?
                    .unwrap_or_default()
                } else {
                    String::new()
                }
            }
        };

        let new_path = join_virtual_path(
            if new_parent_path_str.is_empty() {
                None
            } else {
                Some(new_parent_path_str.as_str())
            },
            &node.name,
        );

        sqlx::query!(
            "UPDATE workspace_nodes SET parent_id = $1, virtual_path = $2, last_modified = now()
             WHERE id = $3 AND tenant_id = $4",
            new_parent.map(|p| p.to_string()),
            new_path,
            node_id.to_string(),
            tenant_id,
        )
        .execute(&self.pool)
        .await?;

        self.get_accessible_node(tenant_id, user_id, node_id).await
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()> {
        // Verify ownership (owner only can delete)
        let exists = sqlx::query_scalar!(
            "SELECT 1 FROM workspace_nodes WHERE id = $1 AND tenant_id = $2 AND owner_id = $3",
            node_id.to_string(),
            tenant_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_none() {
            return Err(anyhow::anyhow!(ConusAiError::NotFound(format!(
                "node {node_id}"
            ))));
        }

        sqlx::query!(
            "DELETE FROM workspace_nodes WHERE id = $1",
            node_id.to_string(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, owner_id))]
    async fn share_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        sqlx::query!(
            "UPDATE workspace_nodes
             SET shared_with = array_append(shared_with, $1), last_modified = now()
             WHERE id = $2 AND tenant_id = $3 AND owner_id = $4",
            with_user_id,
            node_id.to_string(),
            tenant_id,
            owner_id,
        )
        .execute(&self.pool)
        .await?;

        self.get_accessible_node(tenant_id, owner_id, node_id).await
    }

    #[instrument(skip(self), fields(tenant_id, owner_id))]
    async fn unshare_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        sqlx::query!(
            "UPDATE workspace_nodes
             SET shared_with = array_remove(shared_with, $1), last_modified = now()
             WHERE id = $2 AND tenant_id = $3 AND owner_id = $4",
            with_user_id,
            node_id.to_string(),
            tenant_id,
            owner_id,
        )
        .execute(&self.pool)
        .await?;

        self.get_accessible_node(tenant_id, owner_id, node_id).await
    }

    #[instrument(skip(self), fields(tenant_id))]
    async fn bump_last_modified(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE workspace_nodes SET last_modified = now()
             WHERE id = $1 AND tenant_id = $2",
            node_id.to_string(),
            tenant_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, user_id, query))]
    async fn search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let rows = sqlx::query!(
            "SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                    last_modified, shared_with, metadata
             FROM workspace_nodes
             WHERE tenant_id = $1 AND (owner_id = $2 OR $2 = ANY(shared_with))
               AND to_tsvector('english', name || ' ' || virtual_path) @@ plainto_tsquery($3)
             ORDER BY last_modified DESC
             LIMIT $4",
            tenant_id,
            user_id,
            query,
            limit as i64,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| WorkspaceNode {
                id: r.id.parse().unwrap_or_else(|_| Ulid::new()),
                tenant_id: r.tenant_id,
                owner_id: r.owner_id,
                parent_id: r.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(&r.kind),
                name: r.name,
                virtual_path: r.virtual_path,
                last_modified: r.last_modified,
                shared_with: r.shared_with,
                metadata: r.metadata,
            })
            .collect())
    }

    #[instrument(skip(self, content), fields(tenant_id))]
    async fn index_content(
        &self,
        _tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> anyhow::Result<()> {
        // Chunk: 4096 chars max per chunk, up to 4 chunks.
        const CHUNK_SIZE: usize = 4096;
        const MAX_CHUNKS: usize = 4;

        let chars: Vec<char> = content.chars().collect();
        let chunks: Vec<String> = chars
            .chunks(CHUNK_SIZE)
            .take(MAX_CHUNKS)
            .map(|c| c.iter().collect())
            .collect();

        // Embed all chunks in one batch call.
        let embeddings_result = self.embedding_svc.embed_documents(chunks.clone()).await;

        match embeddings_result {
            Ok(embeddings) => {
                for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                    let id = format!("{node_id}_{i}");
                    if let Err(e) = self
                        .vector_store
                        .upsert_content_embedding(
                            &id,
                            &node_id.to_string(),
                            i as i32,
                            chunk,
                            embedding,
                        )
                        .await
                    {
                        warn!(node_id = %node_id, chunk = i, error = %e, "failed to upsert content embedding");
                        // Fall back to plain-text upsert for this chunk.
                        sqlx::query(
                            "INSERT INTO content_embeddings (id, node_id, chunk_index, content, updated_at) \
                             VALUES ($1, $2, $3, $4, now()) \
                             ON CONFLICT (id) DO UPDATE \
                               SET content = EXCLUDED.content, updated_at = now()",
                        )
                        .bind(&id)
                        .bind(node_id.to_string())
                        .bind(i as i32)
                        .bind(chunk.as_str())
                        .execute(&self.pool)
                        .await?;
                    }
                }
            }
            Err(e) => {
                warn!(node_id = %node_id, error = %e, "embedding generation failed; storing plain text only");
                // Store first chunk as plain text without embedding.
                let truncated: String = content.chars().take(CHUNK_SIZE).collect();
                let id = format!("{node_id}_0");
                sqlx::query(
                    "INSERT INTO content_embeddings (id, node_id, chunk_index, content, updated_at) \
                     VALUES ($1, $2, 0, $3, now()) \
                     ON CONFLICT (id) DO UPDATE \
                       SET content = EXCLUDED.content, updated_at = now()",
                )
                .bind(&id)
                .bind(node_id.to_string())
                .bind(&truncated)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, user_id, query))]
    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let embedding = self.embedding_svc.embed_query(query).await?;
        let hits = self
            .vector_store
            .top_n_content(&embedding, tenant_id, user_id, limit)
            .await?;

        Ok(hits
            .into_iter()
            .map(|h| WorkspaceNode {
                id: h.node_id.parse().unwrap_or_else(|_| Ulid::new()),
                tenant_id: h.tenant_id,
                owner_id: h.owner_id,
                parent_id: h.parent_id.as_deref().and_then(|s| s.parse().ok()),
                kind: Self::parse_kind(&h.kind),
                name: h.name,
                virtual_path: h.virtual_path,
                last_modified: h.last_modified,
                shared_with: h.shared_with,
                metadata: h.metadata,
            })
            .collect())
    }

    #[instrument(skip(self), fields(tenant_id))]
    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        sqlx::query!(
            "UPDATE workspace_nodes
             SET metadata = jsonb_set(
                 COALESCE(metadata, '{}'::jsonb),
                 '{thread_id}',
                 to_jsonb($1::text)
             ), last_modified = now()
             WHERE id = $2 AND tenant_id = $3",
            thread_id,
            node_id.to_string(),
            tenant_id,
        )
        .execute(&self.pool)
        .await?;

        // Return node accessible to any user (admin-level bind)
        let row = sqlx::query!(
            "SELECT id, tenant_id, owner_id, parent_id, kind, name, virtual_path,
                    last_modified, shared_with, metadata
             FROM workspace_nodes WHERE id = $1 AND tenant_id = $2",
            node_id.to_string(),
            tenant_id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(WorkspaceNode {
            id: row.id.parse().unwrap_or_else(|_| Ulid::new()),
            tenant_id: row.tenant_id,
            owner_id: row.owner_id,
            parent_id: row.parent_id.as_deref().and_then(|s| s.parse().ok()),
            kind: Self::parse_kind(&row.kind),
            name: row.name,
            virtual_path: row.virtual_path,
            last_modified: row.last_modified,
            shared_with: row.shared_with,
            metadata: row.metadata,
        })
    }
}
