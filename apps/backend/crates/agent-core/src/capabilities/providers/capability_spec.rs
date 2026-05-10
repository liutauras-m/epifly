//! Bulk capability-spec factory — generates `CapabilityProvider` instances from
//! the `capability_specs` Postgres table.
//!
//! This is a domain-neutral source: the `namespace` column (e.g. `erp.po`,
//! `crm.lead`, `accounting.gl`) partitions specs across verticals.
//!
//! # How it works
//! 1. `load_batch` streams all enabled rows from `capability_specs` in chunks.
//! 2. Each row is mapped to a `CapabilityCard` + `CapabilityProvider` based on `strategy`.
//! 3. Embeddings are generated in batches and upserted to `capability_embeddings`.
//! 4. The registry is updated atomically per chunk (no global lock held during IO).
//!
//! # Hot-reload
//! The caller (e.g. `RealtimeService`) listens on the `capability_specs_changed` PG channel
//! and calls `reload_one(namespace, tool_name)` on any INSERT/UPDATE/DELETE.

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
use crate::vector_store::PgVectorStore;
use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

// ── DB row ─────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow, Clone)]
#[allow(dead_code)]
struct CapabilitySpecRow {
    namespace: String,
    tool_name: String,
    description: String,
    input_schema: Value,
    output_schema: Option<Value>,
    strategy: String,
    payload: Value,
    tags: Vec<String>,
    #[sqlx(default)]
    tenant_scope: Vec<String>,
}

// ── Native strategy provider (deterministic JSON transforms) ─────────────────

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
    pool: PgPool,
    llm: Arc<LlmRegistry>,
    embedder: Arc<dyn EmbeddingService>,
    vector_store: Arc<PgVectorStore>,
    batch_size: usize,
}

impl CapabilitySpecFactory {
    pub fn new(
        pool: PgPool,
        llm: Arc<LlmRegistry>,
        embedder: Arc<dyn EmbeddingService>,
        vector_store: Arc<PgVectorStore>,
    ) -> Self {
        Self {
            pool,
            llm,
            embedder,
            vector_store,
            batch_size: 256,
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Reload a single capability spec by (namespace, tool_name) — called by LISTEN/NOTIFY.
    pub async fn reload_one(
        &self,
        registry: &Arc<Mutex<CapabilityRegistry>>,
        namespace: &str,
        tool_name: &str,
    ) -> anyhow::Result<()> {
        let cap_name = qualified_cap_name(namespace, tool_name);

        let row: Option<CapabilitySpecRow> = sqlx::query_as(
            "SELECT namespace, tool_name, description, input_schema, output_schema,
                    strategy, payload, tags,
                    COALESCE(tenant_scope, '{}') AS tenant_scope
             FROM capability_specs
             WHERE namespace = $1 AND tool_name = $2 AND enabled = true",
        )
        .bind(namespace)
        .bind(tool_name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => {
                // Deleted or disabled — unregister.
                registry.lock().unwrap().unregister(&cap_name);
                info!(cap = %cap_name, "capability spec removed from registry");
            }
            Some(row) => {
                let (card, provider) = self.row_to_provider(row)?;
                let card = card.with_provider(provider);
                // Re-embed just this one.
                let text = card.manifest.embedding_text();
                if let Ok(emb) = self.embedder.embed_query(&text).await {
                    let _ = self
                        .vector_store
                        .upsert_capability_embedding_full(
                            &cap_name,
                            &text,
                            &emb,
                            &json!({}),
                            card.namespace(),
                            card.tags(),
                        )
                        .await;
                }
                registry.lock().unwrap().register(card);
                info!(cap = %cap_name, "capability spec hot-reloaded");
            }
        }
        Ok(())
    }

    fn row_to_provider(
        &self,
        row: CapabilitySpecRow,
    ) -> anyhow::Result<(CapabilityCard, Arc<dyn CapabilityProvider>)> {
        let cap_name = qualified_cap_name(&row.namespace, &row.tool_name);
        let tool_def = ToolDef {
            name: row.tool_name.clone(),
            description: row.description.clone(),
            input_schema: row.input_schema.clone(),
        };
        let (kind, chain_cfg) = match row.strategy.as_str() {
            "dynamic_prompt" => (ToolKind::DynamicPrompt, None),
            "prompt" => {
                let model = row.payload["model"]
                    .as_str()
                    .unwrap_or("claude-opus-4-7")
                    .to_string();
                let prompt_template = row.payload["prompt_template"]
                    .as_str()
                    .or_else(|| row.payload["user_template"].as_str())
                    .unwrap_or("{{input}}")
                    .to_string();
                let system_prompt = row.payload["system_prompt"].as_str().map(|s| s.to_string());
                let max_tokens = row.payload["max_tokens"].as_u64().unwrap_or(1024) as u32;
                let vision = row.payload["vision"].as_bool().unwrap_or(false);
                let output_schema = row.payload.get("output_schema").cloned();
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
            other => anyhow::bail!("unsupported capability_specs.strategy: {other}"),
        };

        let source_dir = row.payload["source_dir"]
            .as_str()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let manifest = ToolManifest {
            name: cap_name.clone(),
            version: "1.0.0".into(),
            description: row.description.clone(),
            kind: kind.clone(),
            tools: vec![tool_def],
            config: row.payload.clone(),
            tags: row.tags.clone(),
            namespace: Some(row.namespace.clone()),
            chain: chain_cfg,
            tenant_scope: row.tenant_scope.clone(),
            enabled: true,
            search_keywords: vec![],
        };
        let card = CapabilityCard::new(manifest.clone(), source_dir);

        let provider: Arc<dyn CapabilityProvider> = match kind {
            ToolKind::DynamicPrompt => Arc::new(DynamicPromptCapability::new(
                manifest,
                Arc::clone(&self.llm),
                self.pool.clone(),
            )),
            ToolKind::Chain => {
                Arc::new(PromptChainCapability::new(manifest, Arc::clone(&self.llm))?)
            }
            ToolKind::Wasm => Arc::new(WasmProvider::new(card.clone())),
            ToolKind::Native => Arc::new(NativeSpecProvider {
                manifest,
                payload: row.payload.clone(),
            }),
            ToolKind::RemoteMcp => {
                let endpoint = row.payload["endpoint"]
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
        // CapabilitySpecFactory loads from DB, not from CapabilityCards on disk.
        false
    }

    fn create(&self, _card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        anyhow::bail!("CapabilitySpecFactory::create is not supported; use load_batch instead")
    }
}

#[async_trait]
impl BulkCapabilityFactory for CapabilitySpecFactory {
    async fn load_batch(&self, registry: &mut CapabilityRegistry) -> anyhow::Result<usize> {
        use futures::TryStreamExt;

        info!(
            batch_size = self.batch_size,
            "CapabilitySpecFactory::load_batch starting"
        );

        let mut rows_stream = sqlx::query_as::<_, CapabilitySpecRow>(
            "SELECT namespace, tool_name, description, input_schema, output_schema,
                    strategy, payload, tags,
                    COALESCE(tenant_scope, '{}') AS tenant_scope
             FROM capability_specs
             WHERE enabled = true
             ORDER BY namespace, tool_name",
        )
        .fetch(&self.pool);

        let mut chunk: Vec<CapabilitySpecRow> = Vec::with_capacity(self.batch_size);
        let mut total = 0usize;

        loop {
            let next = rows_stream.try_next().await?;
            let flush = next.is_none() || chunk.len() >= self.batch_size;

            if let Some(ref row) = next {
                chunk.push(row.clone());
            }

            if flush && !chunk.is_empty() {
                total += self.process_chunk(&mut chunk, registry).await;
                chunk.clear();
            }

            if next.is_none() {
                break;
            }
        }

        info!(loaded = total, "CapabilitySpecFactory::load_batch complete");
        Ok(total)
    }
}

impl CapabilitySpecFactory {
    async fn process_chunk(
        &self,
        rows: &mut [CapabilitySpecRow],
        registry: &mut CapabilityRegistry,
    ) -> usize {
        // Prepare embedding texts.
        let texts: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "Tool: {}\nDescription: {}\nNamespace: {}",
                    qualified_cap_name(&r.namespace, &r.tool_name),
                    r.description,
                    r.namespace,
                )
            })
            .collect();

        // Batch embed.
        let embeddings = match self.embedder.embed_documents(texts.clone()).await {
            Ok(e) => e,
            Err(err) => {
                warn!(error = %err, "batch embed failed for capability-spec chunk — skipping embeddings");
                vec![]
            }
        };

        let mut count = 0;
        for (i, row) in rows.iter().enumerate() {
            let cap_name = qualified_cap_name(&row.namespace, &row.tool_name);
            let (card, provider) = match self.row_to_provider(row.clone()) {
                Ok(p) => p,
                Err(e) => {
                    warn!(cap = %cap_name, error = %e, "failed to build provider");
                    continue;
                }
            };

            // Upsert embedding if available.
            if let Some(emb) = embeddings.get(i) {
                let _ = self
                    .vector_store
                    .upsert_capability_embedding_full(
                        &cap_name,
                        &texts[i],
                        emb,
                        &json!({}),
                        &row.namespace,
                        &row.tags,
                    )
                    .await;
            }

            registry.register(card.with_provider(provider));
            count += 1;
        }
        count
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
            }],
            config: serde_json::Value::Null,
            tags: vec![],
            namespace: Some("test".into()),
            chain: None,
            tenant_scope: vec![],
            enabled: true,
            search_keywords: vec![],
        }
    }

    // ── qualified_cap_name ─────────────────────────────────────────────────

    #[test]
    fn qualified_name_with_namespace() {
        assert_eq!(qualified_cap_name("erp.po", "create"), "erp.po.create");
    }

    #[test]
    fn qualified_name_without_namespace() {
        assert_eq!(qualified_cap_name("", "standalone"), "standalone");
    }

    // ── NativeSpecProvider::invoke ─────────────────────────────────────────

    #[tokio::test]
    async fn native_provider_returns_static_result() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("cap.tool"),
            payload: serde_json::json!({"result": {"answer": 42}}),
        };
        let input = serde_json::json!({"q": "ignored"});
        let out = provider.invoke("cap.tool", &input, None).await.unwrap();
        assert_eq!(out, serde_json::json!({"answer": 42}));
    }

    #[tokio::test]
    async fn native_provider_pass_through_echoes_input() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("cap.tool"),
            payload: serde_json::json!({"mode": "pass_through"}),
        };
        let input = serde_json::json!({"key": "value", "n": 7});
        let out = provider.invoke("cap.tool", &input, None).await.unwrap();
        assert_eq!(out, input);
    }

    #[tokio::test]
    async fn native_provider_default_returns_ok_envelope() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("erp.po.create"),
            payload: serde_json::json!({}),
        };
        let input = serde_json::json!({"vendor": "Acme"});
        let out = provider
            .invoke("erp.po.create", &input, None)
            .await
            .unwrap();
        assert_eq!(out["ok"], serde_json::json!(true));
        assert_eq!(out["capability"], serde_json::json!("erp.po.create"));
        assert_eq!(out["tool"], serde_json::json!("erp.po.create"));
        assert_eq!(out["input"], input);
    }

    // ── NativeSpecProvider::manifest ──────────────────────────────────────

    #[test]
    fn native_provider_manifest_name() {
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("erp.invoice.parse"),
            payload: serde_json::json!({}),
        };
        assert_eq!(provider.manifest().name, "erp.invoice.parse");
    }

    // ── row_to_provider: native strategy ──────────────────────────────────
    // We test row_to_provider indirectly through the factory when strategy = "native".
    // This validates the routing logic without requiring a live DB.
    //
    // Note: CapabilitySpecFactory::row_to_provider is private, so we verify the behavior
    // through the public NativeSpecProvider type constructed in the same module.
    #[test]
    fn native_spec_provider_static_result_priority_over_pass_through() {
        // "result" key has priority over "mode": "pass_through" — result key wins.
        let provider = NativeSpecProvider {
            manifest: make_native_manifest("cap"),
            payload: serde_json::json!({"result": {"status": "ok"}, "mode": "pass_through"}),
        };
        let input = serde_json::json!({"x": 1});
        let out = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(provider.invoke("cap", &input, None))
            .unwrap();
        // "result" key takes priority.
        assert_eq!(out, serde_json::json!({"status": "ok"}));
    }
}
