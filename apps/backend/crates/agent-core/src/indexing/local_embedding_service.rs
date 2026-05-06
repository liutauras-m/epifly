//! Offline embedding backend using fastembed (ONNX, no GPU, no API key needed).
//!
//! Enabled only when compiled with `--features agent-core/local-embeddings`.
//! Activated at runtime via `EMBEDDING_BACKEND=local`.

use crate::indexing::EmbeddingService;
use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tokio::sync::Mutex;
use tracing::info;

/// Default local model: nomic-embed-text-v1.5 (768 dims, no GPU needed).
pub const LOCAL_EMBEDDING_DIMS: usize = 768;

pub struct LocalEmbeddingService {
    inner: Mutex<TextEmbedding>,
    dims: usize,
}

impl LocalEmbeddingService {
    /// Construct with the default model (`nomic-embed-text-v1.5`).
    pub fn new() -> anyhow::Result<Self> {
        Self::with_model(EmbeddingModel::NomicEmbedTextV15, LOCAL_EMBEDDING_DIMS)
    }

    fn with_model(model: EmbeddingModel, dims: usize) -> anyhow::Result<Self> {
        let emb = TextEmbedding::try_new(InitOptions::new(model).with_show_download_progress(true))
            .map_err(|e| anyhow::anyhow!("failed to init local embedding model: {e}"))?;
        Ok(Self {
            inner: Mutex::new(emb),
            dims,
        })
    }

    /// Construct from `EMBEDDING_LOCAL_MODEL` env var (defaults to `nomic-embed-text-v1.5`).
    pub fn from_env() -> anyhow::Result<Self> {
        let model_name = std::env::var("EMBEDDING_LOCAL_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text-v1.5".into());
        info!(model = %model_name, "initialising local fastembed model");
        match model_name.as_str() {
            "nomic-embed-text-v1.5" => Self::new(),
            "bge-m3" => Self::with_model(EmbeddingModel::BGEM3, 1024),
            "all-minilm-l6-v2" => Self::with_model(EmbeddingModel::AllMiniLML6V2, 384),
            other => anyhow::bail!("unknown local embedding model: {other}"),
        }
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut guard = self.inner.lock().await;
        let results = guard
            .embed(vec![text], None)
            .map_err(|e| anyhow::anyhow!("fastembed embed_query failed: {e}"))?;
        let embedding: Vec<f32> = results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("fastembed returned empty result"))?;
        if embedding.len() != self.dims {
            anyhow::bail!(
                "embedding dim mismatch: got {}, expected {} — model mismatch",
                embedding.len(),
                self.dims
            );
        }
        Ok(embedding)
    }

    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let mut guard = self.inner.lock().await;
        let results = guard
            .embed(texts.as_slice(), None)
            .map_err(|e| anyhow::anyhow!("fastembed embed_documents failed: {e}"))?;
        for (i, emb) in results.iter().enumerate() {
            if emb.len() != self.dims {
                anyhow::bail!(
                    "embedding[{i}] dim mismatch: got {}, expected {} — model mismatch",
                    emb.len(),
                    self.dims
                );
            }
        }
        Ok(results)
    }
}
