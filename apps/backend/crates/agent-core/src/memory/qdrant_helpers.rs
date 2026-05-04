//! Shared Qdrant REST helpers — point-ID derivation, zero vector, and a thin HTTP wrapper
//! that centralises the upsert / scroll / patch / delete / get patterns with OTel metrics.
//!
//! All three stores (thread, workspace, audit) were previously duplicating these ~150 lines
//! of boilerplate.  They now hold a `QdrantClient` and delegate.
use common::metrics;
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Instant;
use tracing::{Span, instrument};

pub const VECTOR_DIM: usize = 4;

/// Derive a stable u64 Qdrant point ID from any string key (first 8 bytes of SHA-256).
pub fn point_id(key: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

/// Return a `VECTOR_DIM`-dimensional zero vector.
/// Qdrant is used as a document store here — vectors are placeholders only.
pub fn zero_vec() -> Vec<f32> {
    vec![0.0; VECTOR_DIM]
}

/// Thin Qdrant REST client that records OTel duration + error metrics on every operation.
pub struct QdrantClient {
    http: Client,
    pub base_url: String,
}

impl QdrantClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Ensure `collection` exists, creating it and its payload indexes if not present.
    ///
    /// * `keyword_fields` — indexed with the `"keyword"` schema for fast exact-match filtering.
    /// * `text_fields`    — indexed with a word-tokenised text schema for full-text search.
    pub async fn ensure_collection(
        &self,
        collection: &str,
        keyword_fields: &[&str],
        text_fields: &[&str],
    ) -> anyhow::Result<()> {
        let url = format!("{}/collections/{}", self.base_url, collection);

        if self.http.get(&url).send().await?.status().is_success() {
            return Ok(());
        }

        let res = self
            .http
            .put(&url)
            .json(&json!({ "vectors": { "size": VECTOR_DIM, "distance": "Cosine" } }))
            .send()
            .await?;

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("failed to create Qdrant collection {collection}: {body}");
        }

        let idx_url = format!("{}/collections/{}/index", self.base_url, collection);

        for &field in keyword_fields {
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({ "field_name": field, "field_schema": "keyword" }))
                .send()
                .await;
        }

        for &field in text_fields {
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({
                    "field_name": field,
                    "field_schema": {
                        "type": "text",
                        "tokenizer": "word",
                        "min_token_len": 2,
                        "max_token_len": 128,
                        "lowercase": true
                    }
                }))
                .send()
                .await;
        }

        tracing::info!(collection, "created Qdrant collection");
        Ok(())
    }

    /// PUT a single point into `collection`.
    #[instrument(skip(self, point), fields(db.system = "qdrant", db.operation = "upsert", db.collection = collection, error.type = tracing::field::Empty))]
    pub async fn upsert_point(&self, collection: &str, point: Value) -> anyhow::Result<()> {
        let labels = [
            metrics::kv("operation", "upsert"),
            metrics::kv("collection", collection),
        ];
        let t0 = Instant::now();
        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.base_url, collection
        );
        let res = self
            .http
            .put(&url)
            .json(&json!({ "points": [point] }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("Qdrant upsert failed for {collection}: {body}");
        }
        Ok(())
    }

    /// POST a payload-filtered scroll. Returns the raw points array.
    #[instrument(skip(self, filter), fields(db.system = "qdrant", db.operation = "scroll", db.collection = collection, error.type = tracing::field::Empty))]
    pub async fn scroll_filter(
        &self,
        collection: &str,
        filter: Value,
        limit: usize,
    ) -> anyhow::Result<Vec<Value>> {
        let labels = [
            metrics::kv("operation", "scroll"),
            metrics::kv("collection", collection),
        ];
        let t0 = Instant::now();
        let url = format!("{}/collections/{}/points/scroll", self.base_url, collection);
        let res = self
            .http
            .post(&url)
            .json(&json!({
                "filter": filter,
                "limit": limit,
                "with_payload": true,
                "with_vector": false
            }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("Qdrant scroll failed for {collection}: {body}");
        }
        let body: Value = res.json().await?;
        Ok(body["result"]["points"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// POST a targeted payload SET — merges fields into an existing point without replacing it.
    #[instrument(skip(self, fields), fields(db.system = "qdrant", db.operation = "patch_payload", db.collection = collection, error.type = tracing::field::Empty))]
    pub async fn patch_payload(
        &self,
        collection: &str,
        pid: u64,
        fields: Value,
    ) -> anyhow::Result<()> {
        let labels = [
            metrics::kv("operation", "patch_payload"),
            metrics::kv("collection", collection),
        ];
        let t0 = Instant::now();
        let url = format!(
            "{}/collections/{}/points/payload",
            self.base_url, collection
        );
        let res = self
            .http
            .post(&url)
            .json(&json!({ "payload": fields, "points": [pid] }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("Qdrant patch_payload failed for {collection}: {body}");
        }
        Ok(())
    }

    /// Delete a single point by its numeric ID.
    #[instrument(skip(self), fields(db.system = "qdrant", db.operation = "delete", db.collection = collection, error.type = tracing::field::Empty))]
    pub async fn delete_point(&self, collection: &str, pid: u64) -> anyhow::Result<()> {
        let labels = [
            metrics::kv("operation", "delete"),
            metrics::kv("collection", collection),
        ];
        let t0 = Instant::now();
        let url = format!("{}/collections/{}/points/delete", self.base_url, collection);
        let res = self
            .http
            .post(&url)
            .json(&json!({ "points": [pid] }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("Qdrant delete failed for {collection}: {body}");
        }
        Ok(())
    }

    /// Idempotently create text indexes on an already-existing collection.
    ///
    /// Used by `QdrantWorkspaceStore::ensure_text_indexes` to lazily backfill indexes on
    /// collections that pre-date the full-text search feature.  Qdrant treats duplicate
    /// index creation as a no-op (returns 200), so this is safe to call repeatedly.
    pub async fn add_text_indexes(&self, collection: &str, text_fields: &[&str]) {
        let idx_url = format!("{}/collections/{}/index", self.base_url, collection);
        for &field in text_fields {
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({
                    "field_name": field,
                    "field_schema": {
                        "type": "text",
                        "tokenizer": "word",
                        "min_token_len": 2,
                        "max_token_len": 128,
                        "lowercase": true
                    }
                }))
                .send()
                .await;
        }
    }

    /// GET a single point by numeric ID.  Returns `None` on 404.
    pub async fn get_point(&self, collection: &str, pid: u64) -> anyhow::Result<Option<Value>> {
        let url = format!(
            "{}/collections/{}/points/{}",
            self.base_url, collection, pid
        );
        let res = self.http.get(&url).send().await?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            anyhow::bail!(
                "Qdrant get failed for {collection}: {}",
                res.text().await.unwrap_or_default()
            );
        }
        let body: Value = res.json().await?;
        Ok(Some(body["result"].clone()))
    }
}
