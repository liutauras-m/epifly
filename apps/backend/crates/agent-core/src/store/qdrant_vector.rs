//! QdrantVectorStore — ANN similarity search via Qdrant.
//!
//! Collections
//! - `capability_embeddings`: capability search (tag/namespace payload filters).
//! - `content_embeddings`: workspace content search (tenant_id payload filter).
//!
//! Both use 768-dimensional cosine distance (nomic-embed-text default).

use crate::capabilities::namespace::NamespaceFilter;
use chrono::{DateTime, Utc};
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, CreateCollectionBuilder, Distance, FieldType, Filter,
        CreateFieldIndexCollectionBuilder, KeywordIndexParams, PointStruct, SearchParamsBuilder,
        SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder, VectorsConfigBuilder,
        payload_index_params,
    },
};
use serde_json::Value;
use std::collections::HashMap;
use tracing::instrument;

const CAP_COLLECTION: &str = "capability_embeddings";
const CONTENT_COLLECTION: &str = "content_embeddings";
const DIMS: u64 = 768;

// ── DTOs (same as old PgVectorStore) ─────────────────────────────────────────

pub struct CapabilityHit {
    pub capability_id: String,
    pub content: String,
    pub metadata: Value,
    pub distance: f64,
    pub namespace: String,
    pub tags: Vec<String>,
}

pub struct ContentHit {
    pub node_id: String,
    pub content: String,
    pub distance: f64,
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

pub struct QdrantVectorStore {
    inner: Option<Qdrant>,
}

impl QdrantVectorStore {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = Qdrant::from_url(url).build()?;
        let store = Self { inner: Some(client) };
        store.ensure_collections().await?;
        Ok(store)
    }

    /// Returns a store that always returns empty results — used in test mode.
    pub fn noop() -> Self {
        Self { inner: None }
    }

    async fn ensure_collections(&self) -> anyhow::Result<()> {
        let Some(client) = &self.inner else { return Ok(()) };
        for name in [CAP_COLLECTION, CONTENT_COLLECTION] {
            if client.collection_exists(name).await? {
                // Ensure indexes exist even on pre-existing collections.
                self.ensure_payload_indexes(client, name).await?;
                continue;
            }
            let mut vcb = VectorsConfigBuilder::default();
            vcb.add_named_vector_params(
                "default",
                VectorParamsBuilder::new(DIMS, Distance::Cosine),
            );
            client
                .create_collection(
                    CreateCollectionBuilder::new(name).vectors_config(vcb),
                )
                .await?;
            self.ensure_payload_indexes(client, name).await?;
        }
        Ok(())
    }

    async fn ensure_payload_indexes(&self, client: &Qdrant, collection: &str) -> anyhow::Result<()> {
        let tenant_index = CreateFieldIndexCollectionBuilder::new(
            collection,
            "tenant_id",
            FieldType::Keyword,
        )
        .field_index_params(payload_index_params::IndexParams::KeywordIndexParams(
            KeywordIndexParams {
                is_tenant: Some(true),
                on_disk: Some(false),
                enable_hnsw: None,
            },
        ));
        client.create_field_index(tenant_index).await?;

        let owner_index = CreateFieldIndexCollectionBuilder::new(
            collection,
            "owner_id",
            FieldType::Keyword,
        )
        .field_index_params(payload_index_params::IndexParams::KeywordIndexParams(
            KeywordIndexParams {
                is_tenant: Some(false),
                on_disk: Some(false),
                enable_hnsw: None,
            },
        ));
        client.create_field_index(owner_index).await?;

        // shared_with is a repeated string — array keyword index for fast "is_member_of" filters.
        let shared_index = CreateFieldIndexCollectionBuilder::new(
            collection,
            "shared_with",
            FieldType::Keyword,
        )
        .field_index_params(payload_index_params::IndexParams::KeywordIndexParams(
            KeywordIndexParams {
                is_tenant: Some(false),
                on_disk: Some(false),
                enable_hnsw: None,
            },
        ));
        client.create_field_index(shared_index).await?;

        Ok(())
    }

    fn client(&self) -> anyhow::Result<&Qdrant> {
        self.inner
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("QdrantVectorStore is in noop mode"))
    }

    // ── Capability methods ────────────────────────────────────────────────

    pub async fn top_n_capabilities(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<CapabilityHit>> {
        self.top_n_capabilities_filtered(embedding, limit, &NamespaceFilter::Any, &[])
            .await
    }

    #[instrument(skip(self, embedding, tags))]
    pub async fn top_n_capabilities_filtered(
        &self,
        embedding: &[f32],
        limit: usize,
        namespace: &NamespaceFilter,
        tags: &[String],
    ) -> anyhow::Result<Vec<CapabilityHit>> {
        let Ok(client) = self.client() else {
            return Ok(vec![]);
        };

        let filter = build_capability_filter(namespace, tags);

        let mut req = SearchPointsBuilder::new(CAP_COLLECTION, embedding.to_vec(), limit as u64)
            .with_payload(true)
            .params(SearchParamsBuilder::default().exact(false));
        if let Some(f) = filter {
            req = req.filter(f);
        }

        let result = client.search_points(req).await?;
        let hits = result
            .result
            .into_iter()
            .filter_map(|p| {
                let payload = &p.payload;
                let capability_id = payload.get("capability_id")?.as_str()?.as_str().to_string();
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let namespace = payload
                    .get("namespace")
                    .and_then(|v| v.as_str())
                    .map(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let tags: Vec<String> = payload
                    .get("tags")
                    .and_then(|v| v.as_list())
                    .map(|list| {
                        list.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.as_str().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let metadata = payload
                    .get("metadata")
                    .and_then(|v| serde_json::to_value(v).ok())
                    .unwrap_or(Value::Null);
                Some(CapabilityHit {
                    capability_id,
                    content,
                    distance: 1.0 - p.score as f64,
                    namespace,
                    tags,
                    metadata,
                })
            })
            .collect();
        Ok(hits)
    }

    pub async fn upsert_capability_embedding(
        &self,
        capability_id: &str,
        content: &str,
        embedding: &[f32],
        metadata: Value,
    ) -> anyhow::Result<()> {
        self.upsert_capability_embedding_full(capability_id, content, embedding, metadata, "", &[])
            .await
    }

    #[instrument(skip(self, embedding, metadata))]
    pub async fn upsert_capability_embedding_full(
        &self,
        capability_id: &str,
        content: &str,
        embedding: &[f32],
        metadata: Value,
        namespace: &str,
        tags: &[String],
    ) -> anyhow::Result<()> {
        let Ok(client) = self.client() else { return Ok(()) };

        let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
        payload.insert("capability_id".into(), capability_id.into());
        payload.insert("content".into(), content.into());
        payload.insert("namespace".into(), namespace.into());
        payload.insert(
            "tags".into(),
            tags.iter().map(|t| t.as_str()).collect::<Vec<_>>().into(),
        );
        payload.insert("metadata".into(), metadata.to_string().into());

        let mut vectors: HashMap<String, Vec<f32>> = HashMap::new();
        vectors.insert("default".to_string(), embedding.to_vec());

        let point = PointStruct::new(deterministic_id(capability_id), vectors, payload);

        client
            .upsert_points(UpsertPointsBuilder::new(CAP_COLLECTION, vec![point]).wait(true))
            .await?;
        Ok(())
    }

    // ── Content methods ───────────────────────────────────────────────────

    #[instrument(skip(self, embedding))]
    pub async fn top_n_content(
        &self,
        embedding: &[f32],
        limit: usize,
        tenant_id: &str,
    ) -> anyhow::Result<Vec<ContentHit>> {
        let Ok(client) = self.client() else {
            return Ok(vec![]);
        };

        let filter = Filter::must([Condition::matches("tenant_id", tenant_id.to_string())]);

        let result = client
            .search_points(
                SearchPointsBuilder::new(CONTENT_COLLECTION, embedding.to_vec(), limit as u64)
                    .filter(filter)
                    .with_payload(true),
            )
            .await?;

        let hits = result
            .result
            .into_iter()
            .filter_map(|p| {
                let pay = &p.payload;
                Some(ContentHit {
                    node_id: pay.get("node_id")?.as_str()?.as_str().to_string(),
                    content: pay
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    distance: 1.0 - p.score as f64,
                    tenant_id: pay
                        .get("tenant_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    owner_id: pay
                        .get("owner_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    parent_id: pay
                        .get("parent_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str().to_string()),
                    kind: pay
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: pay
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    virtual_path: pay
                        .get("virtual_path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    last_modified: pay
                        .get("last_modified")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.as_str().parse().ok())
                        .unwrap_or_else(Utc::now),
                    shared_with: vec![],
                    metadata: Value::Null,
                })
            })
            .collect();
        Ok(hits)
    }

    #[instrument(skip(self, embedding))]
    pub async fn upsert_content_embedding(
        &self,
        chunk_id: &str,
        node_id: &str,
        chunk_idx: i32,
        content: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        self.upsert_content_embedding_full(chunk_id, node_id, chunk_idx, content, embedding, "", "", &[]).await
    }

    /// Delete all content embedding chunks for a given document (by `node_id` / virtual path).
    #[instrument(skip(self), fields(doc_id))]
    pub async fn delete_content_embeddings_for_doc(&self, doc_id: &str) -> anyhow::Result<()> {
        let Ok(client) = self.client() else { return Ok(()) };
        use qdrant_client::qdrant::{DeletePointsBuilder, Filter, Condition};
        let filter = Filter::must(vec![Condition::matches("node_id", doc_id.to_string())]);
        client
            .delete_points(
                DeletePointsBuilder::new(CONTENT_COLLECTION)
                    .points(filter)
                    .wait(true),
            )
            .await?;
        Ok(())
    }

    #[instrument(skip(self, embedding))]
    pub async fn upsert_content_embedding_full(
        &self,
        chunk_id: &str,
        node_id: &str,
        chunk_idx: i32,
        content: &str,
        embedding: &[f32],
        tenant_id: &str,
        owner_id: &str,
        shared_with: &[String],
    ) -> anyhow::Result<()> {
        let Ok(client) = self.client() else { return Ok(()) };

        let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
        payload.insert("node_id".into(), node_id.into());
        payload.insert("chunk_idx".into(), (chunk_idx as i64).into());
        payload.insert("content".into(), content.into());
        payload.insert("tenant_id".into(), tenant_id.into());
        payload.insert("owner_id".into(), owner_id.into());
        payload.insert(
            "shared_with".into(),
            shared_with.iter().map(|s| s.as_str()).collect::<Vec<_>>().into(),
        );

        let mut vectors: HashMap<String, Vec<f32>> = HashMap::new();
        vectors.insert("default".to_string(), embedding.to_vec());

        let point = PointStruct::new(deterministic_id(chunk_id), vectors, payload);

        client
            .upsert_points(UpsertPointsBuilder::new(CONTENT_COLLECTION, vec![point]).wait(true))
            .await?;
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn deterministic_id(key: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

fn build_capability_filter(ns: &NamespaceFilter, tags: &[String]) -> Option<Filter> {
    let mut must: Vec<Condition> = vec![];
    match ns {
        NamespaceFilter::Any => {}
        NamespaceFilter::Exact(name) => {
            must.push(Condition::matches("namespace", name.clone()));
        }
        NamespaceFilter::Prefix(prefix) => {
            // Qdrant doesn't support prefix natively — filter client-side if needed.
            let _ = prefix;
        }
        NamespaceFilter::AnyOf(_) => {
            // Union filter — skip server-side filtering; caller applies in-memory.
        }
    }
    for tag in tags {
        must.push(Condition::matches("tags", tag.clone()));
    }
    if must.is_empty() {
        None
    } else {
        Some(Filter::must(must))
    }
}
