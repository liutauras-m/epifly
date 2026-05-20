//! Semantic capability router — resolves a natural-language query to the top-K
//! most relevant `CapabilityProvider` instances using ANN vector search.
//!
//! **North-star principle:** Never send 10k tools to the LLM. Always
//! semantic-prefilter to top-K (≤ 50) capabilities per turn.
//!
//! # Usage
//! ```rust,ignore
//! let router = SemanticCapabilityRouter::builder()
//!     .registry(registry)
//!     .vector_store(vector_store)
//!     .embedder(embedder)
//!     .config(SemanticRouterConfig::builder().top_k(20).build())
//!     .build();
//!
//! // In the agent loop:
//! let tool_defs = router.tool_definitions(user_message, &tenant).await?;
//! // ... send tool_defs to LLM ...
//! let result = router.invoke("cap__tool", &input, Some(&tenant)).await?;
//! ```

use crate::capabilities::namespace::NamespaceFilter;
use crate::capabilities::provider::CapabilityProvider;
use crate::capabilities::registry::CapabilityRegistry;
use crate::context::tenant::TenantContext;
use crate::indexing::EmbeddingService;
use crate::store::qdrant_vector::QdrantVectorStore;
use common::metrics;
use rig::completion::ToolDefinition;
use rig::tool::{ToolDyn, ToolError};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{Span, instrument};

// ── AttachmentHint ────────────────────────────────────────────────────────────

/// Set of MIME types present in the current turn's attachments.
///
/// When provided to `select()` / `tool_definitions()`, the router post-filters
/// ANN hits to only those capabilities whose `accepts` list contains at least one
/// glob that matches one of the hint MIME types.  Capabilities with an empty
/// `accepts` list are always kept (they don't declare any MIME restriction).
///
/// The optional `cost_bias` field (`"cheap"` / `"standard"` / `"premium"`) is
/// included in the cache key (Phase 2.1a) so planners can request cost-aware
/// routing without polluting unbiased queries' cached results.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachmentHint {
    /// Sorted, deduplicated MIME types (e.g. `["application/pdf", "image/png"]`).
    mimes: BTreeSet<String>,
    /// Optional cost tier bias for planner-aware ranking (`"cheap"` / `"standard"` / `"premium"`).
    pub cost_bias: Option<String>,
}

impl AttachmentHint {
    pub fn new(mimes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            mimes: mimes.into_iter().map(|m| m.into()).collect(),
            cost_bias: None,
        }
    }

    /// Attach a cost-bias to an existing hint (builder-style).
    pub fn with_cost_bias(mut self, bias: impl Into<String>) -> Self {
        self.cost_bias = Some(bias.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.mimes.is_empty()
    }

    /// Returns `true` when at least one `accepts` glob matches one attachment MIME.
    pub fn matches_any(&self, accepts: &[crate::capabilities::manifest::AcceptSpec]) -> bool {
        if accepts.is_empty() {
            return true; // no restriction declared
        }
        for accept in accepts {
            let pattern = &accept.mime;
            for mime in &self.mimes {
                if mime_matches(pattern, mime) {
                    return true;
                }
            }
        }
        false
    }

    /// Stable bytes for cache key hashing (includes cost_bias so biased queries
    /// don't collide with unbiased ones in the moka cache).
    fn cache_bytes(&self) -> Vec<u8> {
        let mut out = self.mimes.iter().cloned().collect::<Vec<_>>().join(",").into_bytes();
        if let Some(bias) = &self.cost_bias {
            out.push(b'\x01');
            out.extend_from_slice(bias.as_bytes());
        }
        out
    }
}

/// Match a MIME glob pattern (`image/*`, `*`, `application/pdf`) against a concrete MIME.
fn mime_matches(pattern: &str, mime: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        // e.g. "image/*" matches "image/png"
        return mime.starts_with(&format!("{prefix}/"));
    }
    // Exact match (case-insensitive per RFC 2045).
    pattern.eq_ignore_ascii_case(mime)
}

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for `SemanticCapabilityRouter`.
#[derive(Debug, Clone)]
pub struct SemanticRouterConfig {
    /// Hard max capabilities sent to the LLM per turn (absolute cap: 50).
    pub top_k: usize,
    /// Namespace filter applied before ANN search.
    pub namespace: NamespaceFilter,
    /// Only return capabilities that have at least one of these tags (empty = no restriction).
    pub tags_any: Vec<String>,
    /// Maximum cosine distance [0, 2] — hits beyond this are dropped.
    pub max_distance: f64,
    /// Capability names that are ALWAYS included, bypassing semantic scoring.
    pub include_always: Vec<String>,
    /// How long (seconds) to cache the embedding + top-K result for a given query key.
    pub cache_ttl_secs: u64,
}

impl Default for SemanticRouterConfig {
    fn default() -> Self {
        Self {
            top_k: 20,
            namespace: NamespaceFilter::Any,
            tags_any: vec![],
            // nomic-embed-text cosine distances: related ≈ 0.10-0.30, unrelated > 0.40.
            // 0.38 keeps high-confidence matches and drops spurious cross-lingual hits.
            max_distance: 0.38,
            include_always: vec![],
            cache_ttl_secs: 60,
        }
    }
}

// ── Metrics ───────────────────────────────────────────────────────────────────

/// Lightweight per-instance counters (no global registry overhead).
#[derive(Default, Debug)]
pub struct RouterMetrics {
    pub cache_hits: std::sync::atomic::AtomicU64,
    pub cache_misses: std::sync::atomic::AtomicU64,
    pub total_selects: std::sync::atomic::AtomicU64,
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Cache value: resolved top-K capability names + their tool definitions.
#[derive(Clone)]
struct CachedResult {
    cap_names: Vec<String>,
    tool_defs: Vec<Value>,
}

pub struct SemanticCapabilityRouter {
    registry: Arc<Mutex<CapabilityRegistry>>,
    vector_store: Arc<QdrantVectorStore>,
    embedder: Arc<dyn EmbeddingService>,
    cfg: SemanticRouterConfig,
    /// blake3-keyed moka cache: cache_key → CachedResult
    cache: moka::future::Cache<[u8; 32], Arc<CachedResult>>,
    pub metrics: Arc<RouterMetrics>,
}

impl SemanticCapabilityRouter {
    pub fn new(
        registry: Arc<Mutex<CapabilityRegistry>>,
        vector_store: Arc<QdrantVectorStore>,
        embedder: Arc<dyn EmbeddingService>,
        cfg: SemanticRouterConfig,
    ) -> Arc<Self> {
        let ttl = Duration::from_secs(cfg.cache_ttl_secs);
        let cache = moka::future::Cache::builder()
            .max_capacity(4096)
            .time_to_live(ttl)
            .build();
        Arc::new(Self {
            registry,
            vector_store,
            embedder,
            cfg,
            cache,
            metrics: Arc::new(RouterMetrics::default()),
        })
    }

    // ── Cache helpers ─────────────────────────────────────────────────────

    fn cache_key(&self, tenant_id: &str, query: &str, hint: &AttachmentHint) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(tenant_id.as_bytes());
        h.update(b"\x00");
        h.update(query.as_bytes());
        h.update(b"\x00");
        h.update(&self.cfg.top_k.to_le_bytes());
        h.update(&self.cfg.max_distance.to_le_bytes());
        // Stable hash contribution from namespace + tags.
        let ns_repr = format!("{:?}", self.cfg.namespace);
        h.update(ns_repr.as_bytes());
        for t in &self.cfg.tags_any {
            h.update(t.as_bytes());
            h.update(b",");
        }
        // Include attachment hint in cache key.
        h.update(b"\xff");
        h.update(&hint.cache_bytes());
        *h.finalize().as_bytes()
    }

    // ── Core API ──────────────────────────────────────────────────────────

    /// Resolve top-K capability names for `query` (may use cache).
    ///
    /// `hint` — optional set of attachment MIME types.  When provided, ANN hits
    /// whose `accepts` list doesn't contain any matching glob are dropped.
    /// Capabilities with an empty `accepts` list are always kept.
    pub async fn select(
        &self,
        query: &str,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Vec<Arc<dyn CapabilityProvider>>> {
        self.select_with_hint(query, tenant, &AttachmentHint::default()).await
    }

    /// Like `select` but with an explicit [`AttachmentHint`] for MIME post-filtering.
    #[instrument(skip(self, tenant, hint), fields(
        tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or(""),
        top_k = tracing::field::Empty,
        cache_hit = tracing::field::Empty,
    ))]
    pub async fn select_with_hint(
        &self,
        query: &str,
        tenant: Option<&TenantContext>,
        hint: &AttachmentHint,
    ) -> anyhow::Result<Vec<Arc<dyn CapabilityProvider>>> {
        let tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("");
        let key = self.cache_key(tenant_id, query, hint);

        let cached = self.cache.get(&key).await;
        if let Some(result) = cached {
            self.metrics
                .cache_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Span::current().record("cache_hit", true);
            Span::current().record("top_k", result.cap_names.len());
            self.metrics
                .total_selects
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            metrics::semantic_router_cache_hits().add(1, &[]);
            metrics::semantic_router_top_k().record(result.cap_names.len() as u64, &[]);
            let registry = self.registry.lock().unwrap();
            return Ok(self.providers_for_names(&registry, &result.cap_names));
        }

        self.metrics
            .cache_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Span::current().record("cache_hit", false);

        let t0 = Instant::now();

        // Embed the query.
        let embedding = self.embedder.embed_query(query).await?;

        // ANN search with namespace + tags filter.
        let limit = self.cfg.top_k.min(50);
        let hits = self
            .vector_store
            .top_n_capabilities_filtered(&embedding, limit, &self.cfg.namespace, &self.cfg.tags_any)
            .await
            .unwrap_or_default();

        // Apply distance threshold.
        let mut cap_names: Vec<String> = hits
            .into_iter()
            .filter(|h| {
                if h.distance <= self.cfg.max_distance {
                    metrics::semantic_router_distance().record(h.distance, &[]);
                    true
                } else {
                    false
                }
            })
            .map(|h| h.capability_id)
            .collect();

        // Post-filter by AttachmentHint (skip if hint is empty — no attachments).
        if !hint.is_empty() {
            let registry = self.registry.lock().unwrap();
            cap_names.retain(|name| {
                registry
                    .get(name)
                    .map(|card| hint.matches_any(&card.manifest.accepts))
                    .unwrap_or(true) // unknown capability — keep it
            });
        }

        // Enforce tenant_scope — filter out capabilities not visible to this tenant.
        if let Some(t) = tenant {
            let registry = self.registry.lock().unwrap();
            cap_names.retain(|name| {
                registry
                    .get(name)
                    .map(|card| card.is_visible_to(&t.tenant_id))
                    .unwrap_or(true) // if card not in registry yet, let it through
            });
        }

        // Always-include overrides (prepended, deduped).
        for always in &self.cfg.include_always {
            if !cap_names.contains(always) {
                cap_names.insert(0, always.clone());
            }
        }

        Span::current().record("top_k", cap_names.len());
        self.metrics
            .total_selects
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        metrics::semantic_router_top_k().record(cap_names.len() as u64, &[]);
        metrics::capability_router_select_seconds().record(t0.elapsed().as_secs_f64(), &[]);

        // Build tool definitions for cache.
        let tool_defs = {
            let registry = self.registry.lock().unwrap();
            self.build_tool_defs(&registry, &cap_names)
        };

        let result = Arc::new(CachedResult {
            cap_names: cap_names.clone(),
            tool_defs,
        });
        self.cache.insert(key, result).await;

        let registry = self.registry.lock().unwrap();
        Ok(self.providers_for_names(&registry, &cap_names))
    }

    /// Return Anthropic-format tool definitions for the top-K capabilities matching `query`.
    pub async fn tool_definitions(
        &self,
        query: &str,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Vec<Value>> {
        self.tool_definitions_with_hint(query, tenant, &AttachmentHint::default()).await
    }

    /// Like `tool_definitions` but with an explicit `AttachmentHint`.
    pub async fn tool_definitions_with_hint(
        &self,
        query: &str,
        tenant: Option<&TenantContext>,
        hint: &AttachmentHint,
    ) -> anyhow::Result<Vec<Value>> {
        let tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("");
        let key = self.cache_key(tenant_id, query, hint);

        if let Some(result) = self.cache.get(&key).await {
            return Ok(result.tool_defs.clone());
        }

        // Populate cache via select_with_hint() which also builds tool_defs.
        let _ = self.select_with_hint(query, tenant, hint).await?;

        // Now the cache should be populated; fall back to empty if not.
        Ok(self
            .cache
            .get(&key)
            .await
            .map(|r| r.tool_defs.clone())
            .unwrap_or_default())
    }

    /// Build Rig `ToolDyn` instances for the top-K tools selected for a prompt.
    ///
    /// This bridges semantic routing into Rig's dynamic tool system for embedded
    /// `AgentBuilder` usage.
    pub async fn rig_tools_for_prompt(
        self: &Arc<Self>,
        query: &str,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Vec<Box<dyn ToolDyn>>> {
        self.rig_tools_for_prompt_with_hint(query, tenant, &AttachmentHint::default()).await
    }

    /// Like `rig_tools_for_prompt` but with an explicit `AttachmentHint`.
    pub async fn rig_tools_for_prompt_with_hint(
        self: &Arc<Self>,
        query: &str,
        tenant: Option<&TenantContext>,
        hint: &AttachmentHint,
    ) -> anyhow::Result<Vec<Box<dyn ToolDyn>>> {
        let defs = self.tool_definitions_with_hint(query, tenant, hint).await?;
        let tenant = tenant.cloned();

        let tools: Vec<Box<dyn ToolDyn>> = defs
            .into_iter()
            .filter_map(|def| {
                let name = def.get("name").and_then(|v| v.as_str())?.to_string();
                let description = def
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let parameters = def
                    .get("parameters")
                    .cloned()
                    .or_else(|| def.get("input_schema").cloned())
                    .unwrap_or_else(|| serde_json::json!({"type":"object"}));

                Some(Box::new(RigRouterTool {
                    router: Arc::clone(self),
                    tenant: tenant.clone(),
                    name,
                    description,
                    parameters,
                }) as Box<dyn ToolDyn>)
            })
            .collect();

        Ok(tools)
    }

    /// Invoke a named tool (formatted as `"{cap_name}__{tool_name}"`).
    #[instrument(skip(self, input, tenant), fields(
        tool_name = full_tool_name,
        tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or(""),
    ))]
    pub async fn invoke(
        &self,
        full_tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let t0 = Instant::now();
        let labels = [metrics::kv("gen_ai.tool.name", full_tool_name.to_string())];
        metrics::gen_ai_tool_calls().add(1, &labels);

        // Expect format: cap_name__tool_name
        let (cap_name, tool_name) = parse_tool_name(full_tool_name)?;
        let provider = {
            let registry = self.registry.lock().unwrap();
            // 1. Exact lookup (covers TOML capabilities like `runtime-echo`).
            // 2. Dot-restore fallback: Anthropic tool names cannot contain dots, so
            //    `tool_definitions_from_manifest` replaces `.` → `_` in the capability
            //    prefix.  When the LLM echoes the sanitised name back we scan the registry
            //    for the capability whose original name sanitises to `cap_name`.
            //    (A naïve `replace('_', '.')` fails when the name contains real underscores,
            //    e.g. `media_time_get_current_time` ≠ `media.time.get.current.time`.)
            let card = registry.get(cap_name).or_else(|| {
                registry
                    .all()
                    .find(|card| card.manifest.name.replace('.', "_") == cap_name)
            });

            // Enforce tenant visibility before resolving the provider.
            if let (Some(t), Some(c)) = (tenant, &card)
                && !c.is_visible_to(&t.tenant_id)
            {
                return Err(anyhow::anyhow!("no provider for capability '{cap_name}'"));
            }

            card.and_then(|c| registry.get_provider(&c.manifest.name))
                .ok_or_else(|| anyhow::anyhow!("no provider for capability '{cap_name}'"))?
        };

        let invoke_labels = [
            metrics::kv("capability", cap_name.to_string()),
            metrics::kv(
                "kind",
                format!("{:?}", provider.manifest().kind).to_ascii_lowercase(),
            ),
        ];

        let result = provider.invoke(tool_name, input, tenant).await;
        metrics::capability_invoke_seconds().record(t0.elapsed().as_secs_f64(), &invoke_labels);
        result
    }

    // ── Internals ─────────────────────────────────────────────────────────

    fn providers_for_names(
        &self,
        registry: &CapabilityRegistry,
        names: &[String],
    ) -> Vec<Arc<dyn CapabilityProvider>> {
        names
            .iter()
            .filter_map(|n| registry.get_provider(n))
            .collect()
    }

    fn build_tool_defs(&self, registry: &CapabilityRegistry, names: &[String]) -> Vec<Value> {
        names
            .iter()
            .filter_map(|n| registry.get_provider(n))
            .flat_map(|p| p.tool_definitions())
            .collect()
    }

    /// Invalidate all cache entries — call after a capability reload.
    pub async fn invalidate_all(&self) {
        self.cache.invalidate_all();
    }
}

struct RigRouterTool {
    router: Arc<SemanticCapabilityRouter>,
    tenant: Option<TenantContext>,
    name: String,
    description: String,
    parameters: Value,
}

impl ToolDyn for RigRouterTool {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn definition<'a>(
        &'a self,
        _prompt: String,
    ) -> rig::wasm_compat::WasmBoxedFuture<'a, ToolDefinition> {
        Box::pin(async move {
            ToolDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                parameters: self.parameters.clone(),
            }
        })
    }

    fn call<'a>(
        &'a self,
        args: String,
    ) -> rig::wasm_compat::WasmBoxedFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&args)?;
            let output = self
                .router
                .invoke(&self.name, &input, self.tenant.as_ref())
                .await
                .map_err(|e| {
                    ToolError::ToolCallError(Box::new(std::io::Error::other(e.to_string())))
                })?;

            match output {
                Value::String(text) => Ok(text),
                value => Ok(value.to_string()),
            }
        })
    }
}

// ── Tool name parsing ─────────────────────────────────────────────────────────

/// Split `"cap__tool"` → `("cap", "tool")`.
fn parse_tool_name(full: &str) -> anyhow::Result<(&str, &str)> {
    full.split_once("__")
        .ok_or_else(|| anyhow::anyhow!("tool name '{full}' must contain '__' separator"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::manifest::{ToolDef, ToolKind, ToolManifest};
    use crate::capabilities::registry::CapabilityRegistry;
    use crate::context::tenant::{PlanTier, TenantContext};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // ── Mock EmbeddingService ─────────────────────────────────────────────

    struct ConstEmbedder(Vec<f32>);

    #[async_trait]
    impl EmbeddingService for ConstEmbedder {
        fn model(&self) -> crate::indexing::EmbeddingModel {
            crate::indexing::EmbeddingModel::MultilingualE5Large
        }
        async fn embed_query(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
            Ok(self.0.clone())
        }
        async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| self.0.clone()).collect())
        }
    }

    fn make_router(include_always: Vec<String>) -> Arc<SemanticCapabilityRouter> {
        let registry = Arc::new(Mutex::new(CapabilityRegistry::new()));
        let vector_store = Arc::new(QdrantVectorStore::noop());
        let embedder: Arc<dyn EmbeddingService> = Arc::new(ConstEmbedder(vec![0.1; 768]));
        let cfg = SemanticRouterConfig {
            include_always,
            ..Default::default()
        };
        SemanticCapabilityRouter::new(registry, vector_store, embedder, cfg)
    }

    fn make_tenant() -> TenantContext {
        TenantContext::new("tenant-test", Some("user-1"), PlanTier::Free, "/tmp")
    }

    // ── Tool name parsing ─────────────────────────────────────────────────

    #[test]
    fn parse_tool_name_ok() {
        let (cap, tool) = parse_tool_name("my_cap__do_thing").unwrap();
        assert_eq!(cap, "my_cap");
        assert_eq!(tool, "do_thing");
    }

    #[test]
    fn parse_tool_name_err() {
        assert!(parse_tool_name("no_separator").is_err());
    }

    // ── select: cache miss increments miss counter ─────────────────────────

    #[tokio::test]
    async fn select_first_call_is_cache_miss() {
        let router = make_router(vec![]);
        let tenant = make_tenant();

        router
            .select("process invoice", Some(&tenant))
            .await
            .unwrap();

        assert_eq!(
            router
                .metrics
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            1,
            "first call must be a cache miss"
        );
        assert_eq!(
            router
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(
            router
                .metrics
                .total_selects
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    // ── select: second identical call is a cache hit ──────────────────────

    #[tokio::test]
    async fn select_second_call_is_cache_hit() {
        let router = make_router(vec![]);
        let tenant = make_tenant();

        router
            .select("process invoice", Some(&tenant))
            .await
            .unwrap();
        router
            .select("process invoice", Some(&tenant))
            .await
            .unwrap();

        assert_eq!(
            router
                .metrics
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            router
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            1,
            "second identical call must hit the cache"
        );
        assert_eq!(
            router
                .metrics
                .total_selects
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    // ── select: different tenants get separate cache entries ─────────────

    #[tokio::test]
    async fn select_different_tenants_separate_cache_entries() {
        let router = make_router(vec![]);
        let t1 = TenantContext::new("tenant-a", Some("u1"), PlanTier::Free, "/tmp");
        let t2 = TenantContext::new("tenant-b", Some("u2"), PlanTier::Free, "/tmp");

        router
            .select("summarize document", Some(&t1))
            .await
            .unwrap();
        router
            .select("summarize document", Some(&t2))
            .await
            .unwrap();

        // Both should be cache misses since tenant IDs differ.
        assert_eq!(
            router
                .metrics
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        assert_eq!(
            router
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    // ── select: invalidate_all resets cache ───────────────────────────────

    #[tokio::test]
    async fn invalidate_all_clears_cache() {
        let router = make_router(vec![]);
        let tenant = make_tenant();

        router
            .select("find suppliers", Some(&tenant))
            .await
            .unwrap();
        router.invalidate_all().await;
        router
            .select("find suppliers", Some(&tenant))
            .await
            .unwrap();

        // After invalidation the second call must be another miss.
        assert_eq!(
            router
                .metrics
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        assert_eq!(
            router
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    // ── select: include_always prepended even with no vector hits ─────────

    #[tokio::test]
    async fn select_include_always_populated() {
        // Register a native provider so it can be resolved.
        let registry = Arc::new(Mutex::new(CapabilityRegistry::new()));
        {
            let manifest = ToolManifest {
                name: "always_cap".into(),
                version: "1.0.0".into(),
                description: "always present".into(),
                kind: ToolKind::Native,
                tools: vec![ToolDef {
                    name: "always_cap__do".into(),
                    description: "do it".into(),
                    input_schema: serde_json::json!({"type":"object"}),
                }],
                config: serde_json::Value::Null,
                tags: vec![],
                namespace: None,
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
            };
            let card = crate::capabilities::card::CapabilityCard::new(
                manifest.clone(),
                std::path::PathBuf::from("."),
            );
            registry.lock().unwrap().register(card);
        }

        let vector_store = Arc::new(QdrantVectorStore::noop());
        let embedder: Arc<dyn EmbeddingService> = Arc::new(ConstEmbedder(vec![0.1; 768]));
        let cfg = SemanticRouterConfig {
            include_always: vec!["always_cap".into()],
            ..Default::default()
        };
        let router = SemanticCapabilityRouter::new(registry, vector_store, embedder, cfg);
        let tenant = make_tenant();

        // Noop vector store returns no hits; but include_always should produce cap_names.
        let caps = router.select("anything", Some(&tenant)).await.unwrap();
        // The registry card has no provider attached (no factory), so providers_for_names returns empty.
        // But the cache key should contain the "always_cap" name.
        // Verify via tool_definitions (will be empty since no provider) — the key point is no panic.
        let defs = router
            .tool_definitions("anything", Some(&tenant))
            .await
            .unwrap();
        let _ = (caps, defs); // must not panic
    }

    // ── invoke: unknown capability returns an error ───────────────────────

    #[tokio::test]
    async fn invoke_unknown_capability_errors() {
        let router = make_router(vec![]);
        let input = serde_json::json!({"q": "test"});
        let result = router.invoke("missing_cap__tool", &input, None).await;
        assert!(
            result.is_err(),
            "invoking unknown capability must return Err"
        );
    }

    // ── invoke: missing __ separator returns error ────────────────────────

    #[tokio::test]
    async fn invoke_bad_tool_name_errors() {
        let router = make_router(vec![]);
        let input = serde_json::json!({});
        let result = router.invoke("no_separator", &input, None).await;
        assert!(result.is_err());
    }

    // ── cache_key is stable (deterministic) ──────────────────────────────

    #[test]
    fn cache_key_is_deterministic() {
        let router = make_router(vec![]);
        let hint = AttachmentHint::default();
        let k1 = router.cache_key("t1", "invoice", &hint);
        let k2 = router.cache_key("t1", "invoice", &hint);
        assert_eq!(k1, k2);
        let k3 = router.cache_key("t1", "receipt", &hint);
        assert_ne!(k1, k3);
        let k4 = router.cache_key("t2", "invoice", &hint);
        assert_ne!(k1, k4);
    }

    // ── AttachmentHint ────────────────────────────────────────────────────

    #[test]
    fn attachment_hint_mime_glob_matching() {
        use crate::capabilities::manifest::AcceptSpec;
        let hint = AttachmentHint::new(["image/png", "application/pdf"]);
        let accepts = vec![
            AcceptSpec { mime: "image/*".into(), max_size_mb: Some(20) },
        ];
        assert!(hint.matches_any(&accepts), "image/* should match image/png");

        let accepts_pdf = vec![
            AcceptSpec { mime: "application/pdf".into(), max_size_mb: None },
        ];
        assert!(hint.matches_any(&accepts_pdf), "application/pdf exact match");

        let accepts_none = vec![
            AcceptSpec { mime: "text/plain".into(), max_size_mb: None },
        ];
        assert!(!hint.matches_any(&accepts_none), "text/plain should not match image or pdf");

        // Empty accepts = no restriction, always passes
        assert!(hint.matches_any(&[]), "empty accepts = always match");
    }

    #[test]
    fn attachment_hint_wildcard_star() {
        use crate::capabilities::manifest::AcceptSpec;
        let hint = AttachmentHint::new(["video/mp4"]);
        let accepts = vec![AcceptSpec { mime: "*".into(), max_size_mb: None }];
        assert!(hint.matches_any(&accepts), "wildcard * matches anything");
    }

    #[test]
    fn cache_key_differs_with_hint() {
        let router = make_router(vec![]);
        let empty = AttachmentHint::default();
        let with_pdf = AttachmentHint::new(["application/pdf"]);
        let k1 = router.cache_key("t1", "invoice", &empty);
        let k2 = router.cache_key("t1", "invoice", &with_pdf);
        assert_ne!(k1, k2, "different hints should produce different cache keys");
    }
}
