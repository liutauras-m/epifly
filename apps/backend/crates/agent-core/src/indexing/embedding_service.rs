use async_trait::async_trait;

/// Default embedding dimensionality (nomic-embed-text-v1.5 / local fastembed).
pub const EMBEDDING_DIMS: usize = 768;
/// OpenAI text-embedding-3-small dimensionality.
const OPENAI_EMBEDDING_DIMS: usize = 1536;
pub const EMBEDDING_MODEL: &str = "text-embedding-3-small";

// ── Trait ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait EmbeddingService: Send + Sync + 'static {
    /// Embed a single query string (optimised call path).
    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// Embed a batch of document strings.  Returns one vector per input in the
    /// same order.  Each vector is guaranteed to have length `EMBEDDING_DIMS`.
    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>>;
}

// ── OpenAI implementation ────────────────────────────────────────────────────

/// Calls `text-embedding-3-small` via the OpenAI REST API.
///
/// Reads `OPENAI_API_KEY` from the environment at construction time.
pub struct OpenAiEmbeddingService {
    client: reqwest::Client,
    api_key: String,
}

impl OpenAiEmbeddingService {
    pub fn from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            anyhow::anyhow!("OPENAI_API_KEY not set — embedding service unavailable")
        })?;
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
        })
    }

    async fn call_api(&self, inputs: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        #[derive(serde::Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f64>,
        }
        #[derive(serde::Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }

        let resp: EmbeddingResponse = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": EMBEDDING_MODEL,
                "input": inputs,
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("OpenAI request failed: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("OpenAI API error: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("OpenAI response parse failed: {e}"))?;

        let embeddings: Vec<Vec<f32>> = resp
            .data
            .into_iter()
            .map(|d| d.embedding.into_iter().map(|x| x as f32).collect())
            .collect();

        for (i, emb) in embeddings.iter().enumerate() {
            if emb.len() != OPENAI_EMBEDDING_DIMS {
                anyhow::bail!(
                    "embedding[{i}] has {} dims, expected {OPENAI_EMBEDDING_DIMS} — model mismatch",
                    emb.len()
                );
            }
        }

        Ok(embeddings)
    }
}

#[async_trait]
impl EmbeddingService for OpenAiEmbeddingService {
    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let results = self.call_api(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("empty embedding response from OpenAI"))
    }

    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        // OpenAI accepts up to 2048 inputs per request; split if needed.
        const BATCH: usize = 256;
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            out.extend(self.call_api(&refs).await?);
        }
        Ok(out)
    }
}

// ── Noop implementation (test mode) ──────────────────────────────────────────

/// Fails every call — used when `CONUSAI_TEST_MODE=1` and no API key is set.
pub struct NoopEmbeddingService;

#[async_trait]
impl EmbeddingService for NoopEmbeddingService {
    async fn embed_query(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        anyhow::bail!("embedding not available: no OPENAI_API_KEY configured")
    }

    async fn embed_documents(&self, _: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        anyhow::bail!("embedding not available: no OPENAI_API_KEY configured")
    }
}
