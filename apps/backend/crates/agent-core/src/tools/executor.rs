use super::card::CapabilityCard;
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
            tenant_id   = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("none"),
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
    pub fn tool_definitions(card: &CapabilityCard) -> Vec<Value> {
        tool_definitions_from_manifest(&card.manifest)
    }
}

/// Shared helper used by `ToolExecutor::tool_definitions` and the default
/// `CapabilityProvider::tool_definitions` impl.
///
/// # Name sanitisation
/// Anthropic's tool-name constraint is `^[a-zA-Z0-9_-]{1,128}$`.
/// Capability names can contain dots (e.g. `media.time.current-time`) which
/// are not allowed.  We replace every `.` with `_` in the capability-name
/// prefix so the combined `{cap}__{tool}` string is always valid.
/// The inverse (replacing `_` → `.`) is applied in `SemanticCapabilityRouter::invoke`
/// as a fallback when an exact registry lookup misses.
pub fn tool_definitions_from_manifest(manifest: &ToolManifest) -> Vec<Value> {
    // Replace dots so the name satisfies Anthropic's ^[a-zA-Z0-9_-]{1,128}$ constraint.
    let safe_cap = manifest.name.replace('.', "_");
    manifest
        .tools
        .iter()
        .map(|t| {
            json!({
                "name": format!("{safe_cap}__{}", t.name),
                "description": t.description,
                "input_schema": t.input_schema
            })
        })
        .collect()
}

/// If `image_path` is a URL, download it to a temp file and return `(Some(temp), temp_path)`.
/// If it's a local path, return `(None, original_path)`.
pub async fn resolve_image_path(image_path: &str) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
    // Rewrite /ui/files/{token} URLs to the gateway-internal address.
    // The frontend embeds window.location.origin (e.g. http://localhost:5175 in dev) but the
    // gateway must fetch the file through its own loopback — /ui/files supports token-only auth.
    let rewritten;
    let image_path = if let Some(path_start) = image_path.find("/ui/files/") {
        let base = std::env::var("GATEWAY_INTERNAL_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        rewritten = format!("{}{}", base, &image_path[path_start..]);
        rewritten.as_str()
    } else {
        image_path
    };

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
