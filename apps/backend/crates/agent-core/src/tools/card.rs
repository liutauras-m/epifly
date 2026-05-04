use super::manifest::ToolManifest;
use super::provider::ToolProvider;
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct RegisteredToolCard {
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
    pub provider: Option<Arc<dyn ToolProvider>>,
}

impl RegisteredToolCard {
    pub fn new(manifest: ToolManifest, source_dir: std::path::PathBuf) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::new_v4(),
            manifest,
            source_dir,
            embedding_id: None,
            enabled: true,
            last_error: None,
            registered_at: now,
            updated_at: now,
            provider: None,
        }
    }

    pub fn with_provider(mut self, provider: Arc<dyn ToolProvider>) -> Self {
        self.provider = Some(provider);
        self
    }
}

/// Backwards-compat type alias — internal code should migrate to `RegisteredToolCard`.
pub type ToolCard = RegisteredToolCard;
