use super::card::ToolCard;
use super::manifest::ToolManifest;
use super::registry::ToolRegistry;
use crate::context::tenant::TenantContext;
use common::metrics;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{Span, instrument};

/// Public executor — dispatches through the provider registry.
pub struct ToolExecutor;

impl ToolExecutor {
    /// Dispatch a tool call through the provider registered for `cap_name`.
    #[instrument(
        skip(registry, input, tenant),
        fields(
            tool.cap    = cap_name,
            tool.name   = tool_name,
            error.type  = tracing::field::Empty,
        )
    )]
    pub async fn invoke(
        registry: &ToolRegistry,
        cap_name: &str,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let t0 = Instant::now();
        let labels = [
            metrics::kv("capability", cap_name),
            metrics::kv("tool", tool_name),
        ];
        metrics::tool_invocations().add(1, &labels);

        let result = Self::dispatch(registry, cap_name, tool_name, input, tenant).await;

        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        metrics::tool_duration_ms().record(elapsed, &labels);

        if let Err(ref e) = result {
            metrics::tool_errors().add(1, &labels);
            metrics::record_error(&Span::current(), e);
        }

        result
    }

    async fn dispatch(
        registry: &ToolRegistry,
        cap_name: &str,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let provider = registry
            .get_provider(cap_name)
            .ok_or_else(|| anyhow::anyhow!("No provider registered for '{cap_name}'"))?;
        provider.invoke(tool_name, input, tenant).await
    }

    /// Build Anthropic-format tool definitions from a tool card (for callers
    /// that still snapshot cards without going through the provider).
    pub fn tool_definitions(card: &ToolCard) -> Vec<Value> {
        tool_definitions_from_manifest(&card.manifest)
    }
}

/// Shared helper used by `ToolExecutor::tool_definitions` and the default
/// `ToolProvider::tool_definitions` impl.
pub fn tool_definitions_from_manifest(manifest: &ToolManifest) -> Vec<Value> {
    manifest
        .tools
        .iter()
        .map(|t| {
            json!({
                "name": format!("{}__{}", manifest.name, t.name),
                "description": t.description,
                "input_schema": t.input_schema
            })
        })
        .collect()
}

/// If `image_path` is a URL, download it to a temp file and return `(Some(temp), temp_path)`.
/// If it's a local path, return `(None, original_path)`.
pub async fn resolve_image_path(image_path: &str) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
    if image_path.starts_with("http://") || image_path.starts_with("https://") {
        let bytes = reqwest::get(image_path)
            .await
            .map_err(|e| anyhow::anyhow!("download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("read body failed: {e}"))?;

        let ext = if image_path.contains(".pdf") {
            "pdf"
        } else if image_path.contains(".jpg") || image_path.contains(".jpeg") {
            "jpg"
        } else {
            "png"
        };

        let tmp = std::env::temp_dir().join(format!("conusai-{}.{ext}", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, &bytes).map_err(|e| anyhow::anyhow!("write temp file failed: {e}"))?;

        Ok((Some(tmp.clone()), tmp))
    } else {
        Ok((None, Path::new(image_path).to_path_buf()))
    }
}
