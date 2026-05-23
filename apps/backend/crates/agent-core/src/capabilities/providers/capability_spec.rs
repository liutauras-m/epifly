//! Bulk capability-spec factory — generates `CapabilityProvider` instances from
//! in-memory specs (loaded from TOML files or registered programmatically).
//!
//! The old Postgres-backed loading has been replaced with redb + Qdrant.
//! `load_batch` is a no-op unless specs are pre-registered via `add_spec`.
//! Embeddings for registered specs are upserted to the Qdrant `capability_embeddings`
//! collection.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{LlmChainConfig, ToolDef, ToolKind, ToolManifest};
use crate::capabilities::provider::{BulkCapabilityFactory, CapabilityFactory, CapabilityProvider};
use crate::capabilities::providers::remote_mcp::RemoteMcpCapability;
use crate::capabilities::providers::wasm::WasmProvider;
use crate::capabilities::registry::CapabilityRegistry;
use crate::chains::dynamic_prompt::DynamicPromptCapability;
use crate::chains::llm_chain::PromptChainCapability;
use crate::context::tenant::TenantContext;
use crate::indexing::EmbeddingService;
use crate::llm::LlmRegistry;
use crate::store::qdrant_vector::QdrantVectorStore;
use async_trait::async_trait;
use common::metrics;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{info, warn};

// ── In-memory spec row ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CapabilitySpec {
    pub namespace: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub strategy: String,
    pub payload: Value,
    pub tags: Vec<String>,
    pub tenant_scope: Vec<String>,
}

// ── Native strategy provider ─────────────────────────────────────────────────

struct NativeSpecProvider {
    manifest: ToolManifest,
    payload: Value,
}

#[async_trait]
impl CapabilityProvider for NativeSpecProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if let Some(obj) = self.payload.get("result") {
            return Ok(obj.clone());
        }
        if self.payload.get("mode").and_then(|v| v.as_str()) == Some("pass_through") {
            return Ok(input.clone());
        }
        Ok(json!({
            "ok": true,
            "capability": self.manifest.name,
            "tool": tool_name,
            "input": input,
        }))
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub struct CapabilitySpecFactory {
    llm: Arc<LlmRegistry>,
    embedder: Arc<dyn EmbeddingService>,
    vector_store: Arc<QdrantVectorStore>,
    batch_size: usize,
    specs: Vec<CapabilitySpec>,
}

impl CapabilitySpecFactory {
    pub fn new(
        llm: Arc<LlmRegistry>,
        embedder: Arc<dyn EmbeddingService>,
        vector_store: Arc<QdrantVectorStore>,
    ) -> Self {
        Self {
            llm,
            embedder,
            vector_store,
            batch_size: 256,
            specs: vec![],
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Register a spec programmatically (replaces DB row insertion).
    pub fn add_spec(&mut self, spec: CapabilitySpec) {
        self.specs.push(spec);
    }

    fn spec_to_provider(
        &self,
        spec: &CapabilitySpec,
    ) -> anyhow::Result<(CapabilityCard, Arc<dyn CapabilityProvider>)> {
        let cap_name = qualified_cap_name(&spec.namespace, &spec.tool_name);
        let tool_def = ToolDef {
            name: spec.tool_name.clone(),
            description: spec.description.clone(),
            input_schema: spec.input_schema.clone(),
            search_keywords: vec![],
            read_before_write: None,
        };
        let (kind, chain_cfg) = match spec.strategy.as_str() {
            "dynamic_prompt" => (ToolKind::DynamicPrompt, None),
            "prompt" => {
                let model = spec.payload["model"]
                    .as_str()
                    .unwrap_or("claude-opus-4-7")
                    .to_string();
                let prompt_template = spec.payload["prompt_template"]
                    .as_str()
                    .or_else(|| spec.payload["user_template"].as_str())
                    .unwrap_or("{{input}}")
                    .to_string();
                let system_prompt = spec.payload["system_prompt"]
                    .as_str()
                    .map(|s| s.to_string());
                let max_tokens = spec.payload["max_tokens"].as_u64().unwrap_or(1024) as u32;
                let vision = spec.payload["vision"].as_bool().unwrap_or(false);
                let output_schema = spec.payload.get("output_schema").cloned();
                (
                    ToolKind::Chain,
                    Some(LlmChainConfig {
                        model,
                        system_prompt,
                        prompt_template,
                        vision,
                        max_tokens,
                        output_schema,
                    }),
                )
            }
            "wasm" => (ToolKind::Wasm, None),
            "native" => (ToolKind::Native, None),
            "remote_mcp" => (ToolKind::RemoteMcp, None),
            other => anyhow::bail!("unsupported capability strategy: {other}"),
        };

        let source_dir = spec.payload["source_dir"]
            .as_str()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let manifest = ToolManifest {
            name: cap_name.clone(),
            version: "1.0.0".into(),
            description: spec.description.clone(),
            kind: kind.clone(),
            tools: vec![tool_def],
            config: spec.payload.clone(),
            tags: spec.tags.clone(),
            namespace: Some(spec.namespace.clone()),
            chain: chain_cfg,
            tenant_scope: spec.tenant_scope.clone(),
            enabled: true,
            search_keywords: vec![],
            schema_version: "1.0".into(),
            category: None,
            accepts: vec![],
            emits: vec![],
            idempotent: true,
            cost_hint: None,
            requires: vec![],
        };
        let card = CapabilityCard::new(manifest.clone(), source_dir);

        let provider: Arc<dyn CapabilityProvider> = match kind {
            ToolKind::DynamicPrompt => Arc::new(DynamicPromptCapability::new(
                manifest,
                Arc::clone(&self.llm),
            )),
            ToolKind::Chain => {
                Arc::new(PromptChainCapability::new(manifest, Arc::clone(&self.llm))?)
            }
            ToolKind::Wasm => Arc::new(WasmProvider::new(card.clone())),
            ToolKind::Native => Arc::new(NativeSpecProvider {
                manifest,
                payload: spec.payload.clone(),
            }),
            ToolKind::RemoteMcp => {
                let endpoint = spec.payload["endpoint"]
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("remote_mcp spec '{}' missing payload.endpoint", cap_name)
                    })?
                    .to_string();
                RemoteMcpCapability::new(manifest, endpoint)
            }
            _ => anyhow::bail!("unsupported tool kind for capability spec"),
        };
        Ok((card, provider))
    }
}

impl CapabilityFactory for CapabilitySpecFactory {
    fn supports(&self, _kind: &ToolKind, _name: &str) -> bool {
        false
    }

    fn create(&self, _card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        anyhow::bail!("CapabilitySpecFactory::create is not supported; use load_batch instead")
    }
}

#[async_trait]
impl BulkCapabilityFactory for CapabilitySpecFactory {
    async fn load_batch(&self, registry: &mut CapabilityRegistry) -> anyhow::Result<usize> {
        if self.specs.is_empty() {
            return Ok(0);
        }

        info!(
            count = self.specs.len(),
            "CapabilitySpecFactory::load_batch starting"
        );

        let texts: Vec<String> = self
            .specs
            .iter()
            .map(|s| {
                format!(
                    "Tool: {}\nDescription: {}\nNamespace: {}",
                    qualified_cap_name(&s.namespace, &s.tool_name),
                    s.description,
                    s.namespace,
                )
            })
            .collect();

        // ── Embedding cache (PR 2.B.3.1): skip already-cached descriptions ──────
        let cache_hits_counter    = metrics::embedding_cache_hits();
        let cache_misses_counter  = metrics::embedding_cache_misses();
        let miss_indices: Vec<usize> = texts
            .iter()
            .enumerate()
            .filter(|(_, t)| registry.cached_embedding(t).is_none())
            .map(|(i, _)| i)
            .collect();

        let miss_texts: Vec<String> = miss_indices.iter().map(|&i| texts[i].clone()).collect();

        // Record hit/miss metrics.
        let n_hits = texts.len().saturating_sub(miss_indices.len()) as u64;
        let n_misses = miss_indices.len() as u64;
        if n_hits > 0   { cache_hits_counter.add(n_hits, &[]); }
        if n_misses > 0 { cache_misses_counter.add(n_misses, &[]); }

        let fresh_embeddings: Vec<Vec<f32>> = if miss_texts.is_empty() {
            vec![]
        } else {
            match self.embedder.embed_documents(miss_texts.clone()).await {
                Ok(e) => e,
                Err(err) => {
                    warn!(error = %err, "batch embed failed for capability-spec chunk — skipping embeddings");
                    vec![]
                }
            }
        };

        // Populate cache with newly computed embeddings.
        for (j, &idx) in miss_indices.iter().enumerate() {
            if let Some(emb) = fresh_embeddings.get(j) {
                registry.cache_embedding(texts[idx].clone(), emb.clone());
            }
        }

        // Build full embeddings vec (cache hits + fresh) indexed by original position.
        // Cloned so we can later borrow `registry` mutably for `register()`.
        let embeddings: Vec<Option<Vec<f32>>> = texts
            .iter()
            .map(|t| registry.cached_embedding(t).cloned())
            .collect();

        let mut count = 0;
        for (i, spec) in self.specs.iter().enumerate() {
            let cap_name = qualified_cap_name(&spec.namespace, &spec.tool_name);
            let (card, provider) = match self.spec_to_provider(spec) {
                Ok(p) => p,
                Err(e) => {
                    warn!(cap = %cap_name, error = %e, "failed to build provider");
                    continue;
                }
            };

            if let Some(ref emb) = embeddings[i] {
                let _ = self
                    .vector_store
                    .upsert_capability_embedding_full(
                        &cap_name,
                        &texts[i],
                        emb,
                        json!({}),
                        &spec.namespace,
                        &spec.tags,
                    )
                    .await;
            }

            registry.register(card.with_provider(provider));
            count += 1;
        }

        info!(loaded = count, "CapabilitySpecFactory::load_batch complete");
        Ok(count)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn qualified_cap_name(namespace: &str, tool_name: &str) -> String {
    if namespace.is_empty() {
        tool_name.to_string()
    } else {
        format!("{namespace}.{tool_name}")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::manifest::{ToolDef, ToolKind, ToolManifest};

    fn make_native_manifest(name: &str) -> ToolManifest {
        ToolManifest {
            name: name.into(),
            version: "1.0.0".into(),
            description: "test native".into(),
            kind: ToolKind::Native,
            tools: vec![ToolDef {
                name: name.into(),
                description: "test".into(),
                input_schema: serde_json::json!({"type": "object"}),
                search_keywords: vec![],
                read_before_write: None,
            }],
            config: serde_json::Value::Null,
            tags: vec![],
            namespace: Some("test".into()),
            chain: None,
            tenant_scope: vec![],
            enabled: true,
            search_keywords: vec![],
            schema_version: "2.0".into(),
            category: None,
            accepts: vec![],
            emits: vec![],
            idempotent: true,
            cost_hint: None,
            requires: vec![],
        }
    }

    #[test]
    fn qualified_name_with_namespace() {
        assert_eq!(qualified_cap_name("erp.po", "create"), "erp.po.create");
    }

    #[test]
    fn qualified_name_without_namespace() {
        assert_eq!(qualified_cap_name("", "standalone"), "standalone");
    }

    #[tokio::test]
    async fn native_provider_returns_static_result() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("cap.tool"),
            payload: serde_json::json!({"result": {"answer": 42}}),
        };
        let out = provider
            .invoke("cap.tool", &serde_json::json!({}), None)
            .await
            .unwrap();
        assert_eq!(out, serde_json::json!({"answer": 42}));
    }

    #[tokio::test]
    async fn native_provider_pass_through_echoes_input() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("cap.tool"),
            payload: serde_json::json!({"mode": "pass_through"}),
        };
        let input = serde_json::json!({"key": "value"});
        let out = provider.invoke("cap.tool", &input, None).await.unwrap();
        assert_eq!(out, input);
    }
}
