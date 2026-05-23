use async_trait::async_trait;

// ── Model catalogue ───────────────────────────────────────────────────────────

/// Supported local embedding models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingModel {
    /// intfloat/multilingual-e5-large — 1024-d, 100+ languages (DEFAULT).
    MultilingualE5Large,
    /// BAAI/bge-small-en-v1.5 — 384-d, English-only opt-in.
    BgeSmallEnV15,
    /// BAAI/bge-m3 — 1024-d, multilingual, no prefix needed.
    BgeM3,
    /// nomic-embed-text-v1.5 — 768-d, English.
    NomicEmbedTextV15,
    /// all-MiniLM-L6-v2 — 384-d, English lightweight.
    AllMiniLML6V2,
}

impl EmbeddingModel {
    pub fn dims(self) -> u64 {
        match self {
            Self::MultilingualE5Large => 1024,
            Self::BgeSmallEnV15 => 384,
            Self::BgeM3 => 1024,
            Self::NomicEmbedTextV15 => 768,
            Self::AllMiniLML6V2 => 384,
        }
    }

    /// Human-readable name used in env vars and metrics labels.
    pub fn name(self) -> &'static str {
        match self {
            Self::MultilingualE5Large => "multilingual-e5-large",
            Self::BgeSmallEnV15 => "bge-small-en-v1.5",
            Self::BgeM3 => "bge-m3",
            Self::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            Self::AllMiniLML6V2 => "all-minilm-l6-v2",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "multilingual-e5-large" => Some(Self::MultilingualE5Large),
            "bge-small-en-v1.5" => Some(Self::BgeSmallEnV15),
            "bge-m3" => Some(Self::BgeM3),
            "nomic-embed-text-v1.5" => Some(Self::NomicEmbedTextV15),
            "all-minilm-l6-v2" => Some(Self::AllMiniLML6V2),
            _ => None,
        }
    }

    /// Parse from `EMBEDDING_LOCAL_MODEL` env var, defaulting to `MultilingualE5Large`.
    pub fn from_env() -> Self {
        std::env::var("EMBEDDING_LOCAL_MODEL")
            .ok()
            .and_then(|v| Self::from_name(&v))
            .unwrap_or(Self::MultilingualE5Large)
    }
}

/// Embedding dimensionality of the default model (multilingual-e5-large).
pub const EMBEDDING_DIMS: usize = 1024;

// ── Trait ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait EmbeddingService: Send + Sync + 'static {
    fn model(&self) -> EmbeddingModel;

    fn dims(&self) -> u64 {
        self.model().dims()
    }

    /// Embed a single query string (optimised call path).
    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// Embed a batch of document strings. Returns one vector per input in the
    /// same order. Each vector is guaranteed to have length `dims()`.
    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>>;
}

// ── Noop implementation (test mode) ──────────────────────────────────────────

pub struct NoopEmbeddingService;

#[async_trait]
impl EmbeddingService for NoopEmbeddingService {
    fn model(&self) -> EmbeddingModel {
        EmbeddingModel::MultilingualE5Large
    }

    async fn embed_query(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        anyhow::bail!(
            "embeddings disabled: gateway not compiled with --features local-embeddings"
        )
    }

    async fn embed_documents(&self, _: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        anyhow::bail!(
            "embeddings disabled: gateway not compiled with --features local-embeddings"
        )
    }
}
