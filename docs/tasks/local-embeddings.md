**Best local embeddings approach in 2026 for ConusAI: `rig-fastembed` + a top open-source model.**

This is the **canonical, idiomatic, zero-API-key, fully offline** path that aligns perfectly with Rig v0.36+ and the v0.3 platform standards. It respects SRP, keeps the `EmbeddingService` trait clean, and gives you production-grade performance without pulling in heavy Candle/ONNX boilerplate yourself.

### Recommended Model (May 2026)
- **Primary recommendation**: `Qwen/Qwen3-Embedding-0.6B` (or the 8B variant if you have GPU) — tops multilingual MTEB, 32k context, instruction-aware, excellent quality/cost.
- **Fast & lightweight alternative**: `nomic-ai/nomic-embed-text-v1.5` or `BAAI/bge-m3` (via fastembed supported list).
- **Ultra-light (edge)**: `google/embeddinggemma-300m` or `sentence-transformers/all-MiniLM-L6-v2`.

`fastembed-rs` already ships optimized ONNX versions of these with quantization — no PyTorch/Candle needed at runtime.

### 1. Add Dependencies (workspace `Cargo.toml`)
```toml
[workspace.dependencies]
rig-core = "0.36"
rig-fastembed = "0.3"          # Rig integration (check latest)
fastembed = "5"                # if you need direct access
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

In `crates/common` or `agent-core`.

### 2. New `LocalEmbeddingService` (SRP-compliant, async_trait)
Place this in `crates/common/src/embeddings.rs` (or a new `embeddings` module).

```rust
use async_trait::async_trait;
use rig_fastembed::FastembedModel; // or your chosen model enum
use rig::embeddings::EmbeddingModel; // Rig's trait for compatibility

use crate::EmbeddingService; // your existing trait

#[derive(Clone)]
pub struct LocalEmbeddingService {
    model: rig_fastembed::TextEmbedding, // or whatever the Rig wrapper exports
}

impl LocalEmbeddingService {
    /// Best default in 2026 — change via config/env for different models
    pub fn new() -> anyhow::Result<Self> {
        let client = rig_fastembed::Client::new();
        let model = client.embedding_model(FastembedModel::Qwen3Embedding0_6B); // or .AllMiniLML6V2Q, .NomicEmbedTextV15, etc.

        Ok(Self { model })
    }

    /// Or factory for config-driven selection
    pub fn from_model_name(name: &str) -> anyhow::Result<Self> {
        // map string -> FastembedModel variant, with fallback
        // ...
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    async fn embed_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text.to_string()]).await?;
        Ok(embeddings.into_iter().next().unwrap_or_default()) // single vector
    }

    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        self.model.embed(texts).await
            .map_err(|e| anyhow::anyhow!("Local embedding failed: {e}"))
    }
}
```

**Dimensionality note**: Update your `EMBEDDING_DIMS` const per model (e.g., 768 for MiniLM, 1024 for BGE-M3, flexible for Qwen3). Rig’s `EmbeddingModel` trait handles normalization/pooling for you.

### 3. Wiring into Agent / Builder (agent-core)
```rust
// In AgentBuilder or a dedicated EmbeddingProvider
pub fn with_local_embeddings(mut self) -> Self {
    let embedding_svc = Arc::new(LocalEmbeddingService::new()?);
    self.embedding_service = Some(embedding_svc);
    self
}

// Or make it selectable via config
#[derive(Clone, Debug)]
pub enum EmbeddingBackend {
    OpenAI,
    Local(QwenVariant), // or enum of supported models
    Noop,
}
```

### 4. Config / Feature Flag (best practice)
```toml
# In agent-core/Cargo.toml
[features]
local-embeddings = ["dep:rig-fastembed"]
```

Env-driven selection (figment):
```rust
let backend = config.extract::<String>("embedding.backend")?;
match backend.as_str() {
    "local" => builder.with_local_embeddings(),
    "openai" => builder.with_openai_embeddings(),
    _ => builder.with_noop_embeddings(),
}
```

