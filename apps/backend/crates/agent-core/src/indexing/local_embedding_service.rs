//! Offline embedding backend using fastembed (ONNX, no GPU, no API key needed).
//!
//! Enabled only when compiled with `--features agent-core/local-embeddings`.
//! Activated at runtime via `EMBEDDING_BACKEND=local`.
//!
//! Default model: `multilingual-e5-large` (1024-dim).
//! The multilingual-e5 family requires `query: ` prefix for queries
//! and `passage: ` prefix for documents to be indexed.
//!
//! # Why raw fastembed rather than rig's FastEmbed adapter
//!
//! Rig 0.36's `rig::embeddings::fastembed::FastEmbed` hard-wires the embedding
//! call without prefix injection. Multilingual-E5 requires `"query: "` /
//! `"passage: "` prefixes for retrieval quality. Using fastembed directly gives
//! us full control over that prefix, at the cost of not going through the Rig
//! embedding trait. The Rig adapter is a thin wrapper anyway.
//!
//! # Environment variables
//!
//! - `EMBEDDING_LOCAL_MODEL`: model name (default `multilingual-e5-large`)
//! - `EMBEDDING_CACHE_DIR`: directory where ONNX model files are cached
//!   (default: fastembed's own default, typically `~/.cache/huggingface`)
//! - `EMBEDDING_MAX_BATCH`: max documents per fastembed batch call (default `256`)

use crate::indexing::EmbeddingService;
use crate::indexing::embedding_service::EmbeddingModel;
use async_trait::async_trait;
use fastembed::{EmbeddingModel as FastEmbedModel, InitOptions, TextEmbedding};
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct LocalEmbeddingService {
    /// Wrapped in `Arc<std::sync::Mutex>` (not tokio) so it can be moved into
    /// `tokio::task::spawn_blocking` closures. Embedding is CPU-bound; holding
    /// a tokio async mutex across it would block the async thread pool.
    inner: Arc<Mutex<TextEmbedding>>,
    model: EmbeddingModel,
    /// Prefix to prepend to query strings (e.g. "query: " for e5 models).
    query_prefix: &'static str,
    /// Prefix to prepend to document strings (e.g. "passage: " for e5 models).
    passage_prefix: &'static str,
    /// Max documents per batch call.
    max_batch: usize,
}

const DEFAULT_MAX_BATCH: usize = 256;

impl LocalEmbeddingService {
    fn build(
        fe_model: FastEmbedModel,
        model: EmbeddingModel,
        query_prefix: &'static str,
        passage_prefix: &'static str,
    ) -> anyhow::Result<Self> {
        let max_batch = std::env::var("EMBEDDING_MAX_BATCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_BATCH);

        let mut opts = InitOptions::new(fe_model).with_show_download_progress(true);
        if let Ok(cache_dir) = std::env::var("EMBEDDING_CACHE_DIR") {
            opts = opts.with_cache_dir(cache_dir.into());
        }

        let emb = TextEmbedding::try_new(opts)
            .map_err(|e| anyhow::anyhow!("failed to init local embedding model: {e}"))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(emb)),
            model,
            query_prefix,
            passage_prefix,
            max_batch,
        })
    }

    /// Construct from the typed `EmbeddingModel` enum.
    pub fn from_model(model: EmbeddingModel) -> anyhow::Result<Self> {
        info!(model = model.name(), "initialising local fastembed model");
        match model {
            EmbeddingModel::MultilingualE5Large => Self::build(
                FastEmbedModel::MultilingualE5Large,
                model,
                "query: ",
                "passage: ",
            ),
            EmbeddingModel::BgeSmallEnV15 => {
                Self::build(FastEmbedModel::BGESmallENV15, model, "", "")
            }
            EmbeddingModel::BgeM3 => Self::build(FastEmbedModel::BGEM3, model, "", ""),
            EmbeddingModel::NomicEmbedTextV15 => {
                Self::build(FastEmbedModel::NomicEmbedTextV15, model, "", "")
            }
            EmbeddingModel::AllMiniLML6V2 => {
                Self::build(FastEmbedModel::AllMiniLML6V2, model, "", "")
            }
        }
    }

    /// Construct from env vars (`EMBEDDING_LOCAL_MODEL`).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_model(EmbeddingModel::from_env())
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    fn model(&self) -> EmbeddingModel {
        self.model
    }

    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let prefixed = format!("{}{text}", self.query_prefix);
        let inner = Arc::clone(&self.inner);
        let expected_dims = self.model.dims() as usize;

        let embedding = tokio::task::spawn_blocking(move || {
            let mut guard = inner
                .lock()
                .map_err(|_| anyhow::anyhow!("fastembed mutex poisoned"))?;
            let results = guard
                .embed(vec![prefixed.as_str()], None)
                .map_err(|e| anyhow::anyhow!("fastembed embed_query failed: {e}"))?;
            results
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("fastembed returned empty result"))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))??;

        if embedding.len() != expected_dims {
            anyhow::bail!(
                "embedding dim mismatch: got {}, expected {} — model mismatch",
                embedding.len(),
                expected_dims
            );
        }
        Ok(embedding)
    }

    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let prefixed: Vec<String> = texts
            .iter()
            .map(|t| format!("{}{t}", self.passage_prefix))
            .collect();

        let inner = Arc::clone(&self.inner);
        let max_batch = self.max_batch;
        let expected_dims = self.model.dims() as usize;

        let all_results = tokio::task::spawn_blocking(move || {
            let mut guard = inner
                .lock()
                .map_err(|_| anyhow::anyhow!("fastembed mutex poisoned"))?;
            let mut all: Vec<Vec<f32>> = Vec::with_capacity(prefixed.len());
            for chunk in prefixed.chunks(max_batch) {
                let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
                let batch = guard
                    .embed(refs, None)
                    .map_err(|e| anyhow::anyhow!("fastembed embed_documents failed: {e}"))?;
                all.extend(batch);
            }
            Ok::<_, anyhow::Error>(all)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))??;

        for (i, emb) in all_results.iter().enumerate() {
            if emb.len() != expected_dims {
                anyhow::bail!(
                    "embedding[{i}] dim mismatch: got {}, expected {} — model mismatch",
                    emb.len(),
                    expected_dims
                );
            }
        }
        Ok(all_results)
    }
}
