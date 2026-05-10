use super::manifest::ToolManifest;
use super::provider::CapabilityProvider;
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct CapabilityCard {
    pub id: Uuid,
    pub manifest: ToolManifest,
    /// Directory on disk where `capability.toml` (and optional `.wasm`) live.
    pub source_dir: std::path::PathBuf,
    pub embedding_id: Option<String>,
    /// Whether this capability is exposed to agents and `/v1/capabilities`.
    pub enabled: bool,
    /// Last error from a factory create or reload attempt.
    pub last_error: Option<String>,
    pub registered_at: SystemTime,
    pub updated_at: SystemTime,
    /// Cached provider — cheap to clone, avoids re-creating on each read.
    pub provider: Option<Arc<dyn CapabilityProvider>>,
}

impl CapabilityCard {
    pub fn new(manifest: ToolManifest, source_dir: std::path::PathBuf) -> Self {
        let now = SystemTime::now();
        let enabled = manifest.enabled;
        Self {
            id: Uuid::new_v4(),
            manifest,
            source_dir,
            embedding_id: None,
            enabled,
            last_error: None,
            registered_at: now,
            updated_at: now,
            provider: None,
        }
    }

    /// Primary namespace (empty string if unnamespaced).
    pub fn namespace(&self) -> &str {
        self.manifest.namespace()
    }

    /// Secondary tags for filtering.
    pub fn tags(&self) -> &[String] {
        &self.manifest.tags
    }

    pub fn with_provider(mut self, provider: Arc<dyn CapabilityProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Returns true if this capability is visible to `tenant_id`.
    /// An empty scope means global (always visible).
    pub fn is_visible_to(&self, tenant_id: &str) -> bool {
        let scope = &self.manifest.tenant_scope;
        scope.is_empty() || scope.iter().any(|t| t == tenant_id)
    }
}
