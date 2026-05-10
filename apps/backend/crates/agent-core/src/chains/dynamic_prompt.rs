//! DB-backed, versioned prompt capability.
//!
//! `DynamicPromptCapability` loads its `LlmChainConfig` from the `dynamic_prompts`
//! Postgres table at invocation time (with a moka cache for hot-path performance).
//! The latest version is used by default; a specific version can be pinned at
//! construction time for rollback / A-B testing.
//!
//! Updating the prompt = `INSERT` a new row; the cache invalidates on the next
//! turn (TTL-based) or immediately via `invalidate()`.

use crate::chains::executor;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::capabilities::manifest::{LlmChainConfig, ToolManifest};
use crate::capabilities::provider::CapabilityProvider;
use async_trait::async_trait;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{Span, debug, instrument};

// ── DB row ────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct PromptRow {
    system_prompt: Option<String>,
    user_template: String,
    model: String,
    max_tokens: i32,
    vision: bool,
    output_schema: Option<Value>,
}

// ── Provider ──────────────────────────────────────────────────────────────────

pub struct DynamicPromptCapability {
    manifest: ToolManifest,
    llm: Arc<LlmRegistry>,
    pool: PgPool,
    /// moka cache keyed by "{capability_name}:{version}" for pinned versions.
    cache: moka::future::Cache<String, Arc<LlmChainConfig>>,
    /// If `Some`, always use this specific version. `None` = latest.
    pinned_version: Option<i32>,
}

impl DynamicPromptCapability {
    pub fn new(manifest: ToolManifest, llm: Arc<LlmRegistry>, pool: PgPool) -> Self {
        let cache = moka::future::Cache::builder()
            .max_capacity(256)
            .time_to_live(Duration::from_secs(60))
            .build();
        Self {
            manifest,
            llm,
            pool,
            cache,
            pinned_version: None,
        }
    }

    pub fn with_pinned_version(mut self, version: i32) -> Self {
        self.pinned_version = Some(version);
        self
    }

    /// Invalidate all cached configs for this capability.
    pub async fn invalidate(&self) {
        self.cache.invalidate_all();
    }

    // ── Internals ─────────────────────────────────────────────────────────

    #[instrument(skip(self), fields(
        capability = %self.manifest.name,
        version = self.pinned_version,
        cache_hit = tracing::field::Empty,
    ))]
    async fn load_latest(&self) -> anyhow::Result<Arc<LlmChainConfig>> {
        if let Some(v) = self.pinned_version {
            let cache_key = format!("{}:{v}", self.manifest.name);
            if let Some(cfg) = self.cache.get(&cache_key).await {
                debug!(capability = %self.manifest.name, version = v, "dynamic prompt cache hit");
                Span::current().record("cache_hit", true);
                return Ok(cfg);
            }
            Span::current().record("cache_hit", false);

            let row: PromptRow = sqlx::query_as(
                "SELECT system_prompt, user_template, model, max_tokens, vision, output_schema
                 FROM dynamic_prompts
                 WHERE capability_name = $1 AND version = $2",
            )
            .bind(&self.manifest.name)
            .bind(v)
            .fetch_one(&self.pool)
            .await?;

            let cfg = Arc::new(LlmChainConfig {
                model: row.model,
                system_prompt: row.system_prompt,
                prompt_template: row.user_template,
                vision: row.vision,
                max_tokens: row.max_tokens as u32,
                output_schema: row.output_schema,
            });
            self.cache.insert(cache_key, Arc::clone(&cfg)).await;
            return Ok(cfg);
        }

        // Latest version is intentionally uncached so admin updates are visible on next turn.
        Span::current().record("cache_hit", false);
        let row: PromptRow = sqlx::query_as(
            "SELECT system_prompt, user_template, model, max_tokens, vision, output_schema
                 FROM dynamic_prompts
                 WHERE capability_name = $1
                 ORDER BY version DESC
                 LIMIT 1",
        )
        .bind(&self.manifest.name)
        .fetch_one(&self.pool)
        .await?;

        let cfg = Arc::new(LlmChainConfig {
            model: row.model,
            system_prompt: row.system_prompt,
            prompt_template: row.user_template,
            vision: row.vision,
            max_tokens: row.max_tokens as u32,
            output_schema: row.output_schema,
        });
        Ok(cfg)
    }
}

#[async_trait]
impl CapabilityProvider for DynamicPromptCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    #[instrument(skip(self, input, tenant), fields(
        tool = %tool_name,
        capability = %self.manifest.name,
        version = self.pinned_version,
    ))]
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let cfg = self.load_latest().await?;
        let tenant_view = tenant
            .map(|t| json!({ "id": &*t.tenant_id, "plan": t.plan.to_string() }))
            .unwrap_or(Value::Null);
        let ctx = json!({ "input": input, "tenant": tenant_view });
        executor::run_chain(&cfg, &ctx, &self.llm, tenant).await
    }
}
