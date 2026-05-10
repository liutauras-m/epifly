use super::registry::CapabilityRegistry;
use std::path::PathBuf;
use tracing::info;

pub struct CapabilityDiscovery {
    dirs: Vec<PathBuf>,
}

impl CapabilityDiscovery {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    pub fn from_env() -> Self {
        let dir = std::env::var("CONUSAI_CAPABILITIES_DIR")
            .unwrap_or_else(|_| "./capabilities".to_string());
        Self::new(vec![PathBuf::from(dir)])
    }

    pub fn discover(&self) -> common::error::Result<CapabilityRegistry> {
        let mut registry = CapabilityRegistry::new();
        self.discover_into(&mut registry)?;
        Ok(registry)
    }

    /// Discover capabilities into an existing registry (preserves pre-registered factories
    /// and providers).  Use with `CapabilityRegistry::with_default_factories()` so YAML-loaded
    /// capabilities receive the correct provider factories.
    pub fn discover_into(&self, registry: &mut CapabilityRegistry) -> common::error::Result<()> {
        let mut total = 0;
        for dir in &self.dirs {
            let count = registry.load_from_dir(dir)?;
            info!(dir = ?dir, count, "discovered tools");
            total += count;
        }
        info!(total, "tool discovery complete");
        Ok(())
    }
}
