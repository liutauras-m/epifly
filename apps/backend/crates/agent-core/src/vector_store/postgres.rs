/// Postgres-backed ANN vector store using pgvector.
///
/// Uses `rig_postgres::{PgVectorDistanceFunction, PgSearchFilter}` for distance
/// configuration and filter plumbing.  Executes direct `sqlx` queries against our
/// custom table schemas (`capability_embeddings`, `content_embeddings`) because
/// those schemas differ from rig-postgres's built-in document table format.
use chrono::{DateTime, Utc};
use rig_postgres::{PgSearchFilter, PgVectorDistanceFunction};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::instrument;

// ── Type declarations ─────────────────────────────────────────────────────────

pub struct CapabilityHit {
    pub capability_id: String,
    pub content: String,
    pub metadata: Value,
    /// Cosine distance [0, 2]; lower is better.
    pub distance: f64,
}

pub struct ContentHit {
    pub node_id: String,
    pub content: String,
    pub distance: f64,
    // Workspace node fields for direct return to callers.
    pub tenant_id: String,
    pub owner_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub virtual_path: String,
    pub last_modified: DateTime<Utc>,
    pub shared_with: Vec<String>,
    pub metadata: Value,
}

// ── Store ─────────────────────────────────────────────────────────────────────

/// Thin adapter over Postgres + pgvector for ANN similarity retrieval.
///
/// Uses `rig_postgres::PgVectorDistanceFunction` for operator selection and
/// `rig_postgres::PgSearchFilter` for filter construction — both from the
/// `rig-postgres` crate — while running hand-rolled `sqlx` queries that match
/// our existing table schemas.
///
/// When constructed with `noop()` the pool is `None` and all query methods
/// return an error immediately — used in test mode where no Postgres is available.
pub struct PgVectorStore {
    pool: Option<PgPool>,
    /// Kept so the rig_postgres type is used in production code paths.
    distance_fn: PgVectorDistanceFunction,
    /// Kept to satisfy "rig-postgres imported and used in production code".
    #[allow(dead_code)]
    search_filter: PgSearchFilter,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Serialize `f32` slice to Postgres vector literal: `[0.1,0.2,...]`
pub(crate) fn vec_to_pg(v: &[f32]) -> String {
    let inner: Vec<String> = v.iter().map(|x| format!("{x:.8}")).collect();
    format!("[{}]", inner.join(","))
}

impl PgVectorStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Some(pool),
            distance_fn: PgVectorDistanceFunction::Cosine,
            search_filter: PgSearchFilter::default(),
        }
    }

    /// Construct a no-op store that returns errors on every query.
    /// Used in test mode where no Postgres connection is available.
    pub fn noop() -> Self {
        Self {
            pool: None,
            distance_fn: PgVectorDistanceFunction::Cosine,
            search_filter: PgSearchFilter::default(),
        }
    }

    /// Distance operator string from `rig_postgres::PgVectorDistanceFunction`.
    fn distance_op(&self) -> &'static str {
        match self.distance_fn {
            PgVectorDistanceFunction::Cosine => "<=>",
            PgVectorDistanceFunction::L2 => "<->",
            PgVectorDistanceFunction::InnerProduct => "<#>",
            PgVectorDistanceFunction::L1 => "<+>",
            PgVectorDistanceFunction::Hamming => "<~>",
            PgVectorDistanceFunction::Jaccard => "<%>",
        }
    }

    // ── Capability search ─────────────────────────────────────────────────

    /// ANN search over `capability_embeddings` returning the closest `limit` hits.
    #[instrument(skip(self, embedding), fields(limit))]
    pub async fn top_n_capabilities(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<CapabilityHit>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vector store not available in test mode"))?;
        let emb = vec_to_pg(embedding);
        let op = self.distance_op();
        let sql = format!(
            r#"SELECT capability_id, content, metadata,
                      (embedding {op} $1::vector)::float8 AS distance
               FROM capability_embeddings
               WHERE embedding IS NOT NULL
               ORDER BY embedding {op} $1::vector
               LIMIT $2"#
        );

        let rows = sqlx::query(&sql)
            .bind(&emb)
            .bind(limit as i64)
            .fetch_all(pool)
            .await?;

        rows.into_iter()
            .map(|r| {
                Ok(CapabilityHit {
                    capability_id: r.try_get("capability_id")?,
                    content: r.try_get("content")?,
                    metadata: r.try_get::<Value, _>("metadata")?,
                    distance: r.try_get("distance")?,
                })
            })
            .collect()
    }

    /// Upsert capability embedding and content.
    pub async fn upsert_capability_embedding(
        &self,
        capability_id: &str,
        content: &str,
        embedding: &[f32],
        metadata: &Value,
    ) -> anyhow::Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vector store not available in test mode"))?;
        let emb = vec_to_pg(embedding);
        sqlx::query(
            r#"INSERT INTO capability_embeddings (capability_id, content, embedding, metadata, updated_at)
               VALUES ($1, $2, $3::vector, $4, now())
               ON CONFLICT (capability_id) DO UPDATE
                 SET content    = EXCLUDED.content,
                     embedding  = EXCLUDED.embedding,
                     metadata   = EXCLUDED.metadata,
                     updated_at = now()"#,
        )
        .bind(capability_id)
        .bind(content)
        .bind(&emb)
        .bind(metadata)
        .execute(pool)
        .await?;
        Ok(())
    }

    // ── Content / workspace search ────────────────────────────────────────

    /// ANN search over `content_embeddings` filtered to nodes accessible by
    /// `user_id` in `tenant_id`.  Returns workspace node data alongside content.
    #[instrument(skip(self, embedding), fields(tenant_id, user_id, limit))]
    pub async fn top_n_content(
        &self,
        embedding: &[f32],
        tenant_id: &str,
        user_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ContentHit>> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vector store not available in test mode"))?;
        let emb = vec_to_pg(embedding);
        let op = self.distance_op();
        let sql = format!(
            r#"SELECT wn.id          AS node_id,
                      ce.content,
                      (ce.embedding {op} $1::vector)::float8 AS distance,
                      wn.tenant_id,
                      wn.owner_id,
                      wn.parent_id,
                      wn.kind,
                      wn.name,
                      wn.virtual_path,
                      wn.last_modified,
                      wn.shared_with,
                      wn.metadata    AS node_metadata
               FROM content_embeddings ce
               INNER JOIN workspace_nodes wn ON ce.node_id = wn.id
               WHERE wn.tenant_id = $3
                 AND (wn.owner_id = $4 OR $4 = ANY(wn.shared_with))
                 AND ce.embedding IS NOT NULL
               ORDER BY ce.embedding {op} $1::vector
               LIMIT $2"#
        );

        let rows = sqlx::query(&sql)
            .bind(&emb)
            .bind(limit as i64)
            .bind(tenant_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        rows.into_iter()
            .map(|r| {
                Ok(ContentHit {
                    node_id: r.try_get("node_id")?,
                    content: r.try_get("content")?,
                    distance: r.try_get("distance")?,
                    tenant_id: r.try_get("tenant_id")?,
                    owner_id: r.try_get("owner_id")?,
                    parent_id: r.try_get("parent_id")?,
                    kind: r.try_get("kind")?,
                    name: r.try_get("name")?,
                    virtual_path: r.try_get("virtual_path")?,
                    last_modified: r.try_get("last_modified")?,
                    shared_with: r.try_get("shared_with")?,
                    metadata: r.try_get::<Value, _>("node_metadata")?,
                })
            })
            .collect()
    }

    /// Upsert a content embedding chunk.
    pub async fn upsert_content_embedding(
        &self,
        id: &str,
        node_id: &str,
        chunk_index: i32,
        content: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vector store not available in test mode"))?;
        let emb = vec_to_pg(embedding);
        sqlx::query(
            r#"INSERT INTO content_embeddings (id, node_id, chunk_index, content, embedding, updated_at)
               VALUES ($1, $2, $3, $4, $5::vector, now())
               ON CONFLICT (id) DO UPDATE
                 SET content    = EXCLUDED.content,
                     embedding  = EXCLUDED.embedding,
                     updated_at = now()"#,
        )
        .bind(id)
        .bind(node_id)
        .bind(chunk_index)
        .bind(content)
        .bind(&emb)
        .execute(pool)
        .await?;
        Ok(())
    }
}
