use super::card::CapabilityCard;
use super::manifest::ToolManifest;
use super::provider::{CapabilityProvider, CapabilityFactory};
use crate::llm::LlmRegistry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{info, warn};

#[derive(Default)]
pub struct ToolRegistry {
    cards: HashMap<String, CapabilityCard>,
    factories: Vec<Box<dyn CapabilityFactory>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_factories(llm: Arc<LlmRegistry>) -> Self {
        use super::providers::{
            builtin::BuiltinFactory, chain::ChainFactory, mcp::McpFactory, wasm::WasmFactory,
        };
        let mut r = Self::new();
        r.register_factory(McpFactory);
        r.register_factory(WasmFactory);
        r.register_factory(ChainFactory::new(llm));
        r.register_factory(BuiltinFactory);
        r
    }

    pub fn register_factory(&mut self, factory: impl CapabilityFactory) {
        self.factories.push(Box::new(factory));
    }

    /// Register a card that already has a provider cached on it.
    pub fn register(&mut self, card: CapabilityCard) {
        info!(name = %card.manifest.name, kind = ?card.manifest.kind, enabled = %card.enabled, "registering tool card");
        self.cards.insert(card.manifest.name.clone(), card);
    }

    /// Register a provider by building a card from its manifest.
    pub fn register_provider(&mut self, provider: Arc<dyn CapabilityProvider>) {
        let manifest = provider.manifest().clone();
        info!(name = %manifest.name, kind = ?manifest.kind, "registering tool provider");
        let card = CapabilityCard::new(manifest, std::path::PathBuf::from("."))
            .with_provider(provider);
        self.cards.insert(card.manifest.name.clone(), card);
    }

    /// Register a card without a provider.
    pub fn register_card(&mut self, card: CapabilityCard) {
        self.register(card);
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Remove a capability. Returns true if it existed.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.cards.remove(name).is_some()
    }

    /// Replace or add a provider in-place.
    pub fn replace(&mut self, provider: Arc<dyn CapabilityProvider>) {
        let name = provider.manifest().name.clone();
        if let Some(card) = self.cards.get_mut(&name) {
            card.provider = Some(provider);
            card.updated_at = SystemTime::now();
            card.last_error = None;
        } else {
            self.register_provider(provider);
        }
    }

    /// Enable or disable a capability. Returns false if not found.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(card) = self.cards.get_mut(name) {
            card.enabled = enabled;
            card.updated_at = SystemTime::now();
            true
        } else {
            false
        }
    }

    /// Reload a single capability directory: re-read TOML, rebuild provider.
    pub fn reload_capability(&mut self, dir: &Path) -> common::error::Result<()> {
        let manifest_path = dir.join("capability.toml");
        let manifest = ToolManifest::from_file(&manifest_path)?;
        let name = manifest.name.clone();
        let mut card = CapabilityCard::new(manifest, dir.to_path_buf());
        // Preserve enabled state.
        card.enabled = self.cards.get(&name).map(|c| c.enabled).unwrap_or(true);
        if let Some(factory) = self.factory_for(&card) {
            match factory.create(card.clone()) {
                Ok(provider) => card.provider = Some(provider),
                Err(e) => card.last_error = Some(e.to_string()),
            }
        }
        self.cards.insert(name, card);
        Ok(())
    }

    // ── Reads ─────────────────────────────────────────────────────────────────

    pub fn get(&self, name: &str) -> Option<&CapabilityCard> {
        self.cards.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut CapabilityCard> {
        self.cards.get_mut(name)
    }

    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn CapabilityProvider>> {
        self.cards.get(name)?.provider.clone()
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&CapabilityCard> {
        self.cards
            .values()
            .filter(|c| c.manifest.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// All cards (enabled and disabled).
    pub fn all(&self) -> impl Iterator<Item = &CapabilityCard> {
        self.cards.values()
    }

    /// Only enabled cards — used by `/v1/capabilities` and agent execution.
    pub fn all_enabled(&self) -> impl Iterator<Item = &CapabilityCard> {
        self.cards.values().filter(|c| c.enabled)
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn factory_for(&self, card: &CapabilityCard) -> Option<&dyn CapabilityFactory> {
        self.factories
            .iter()
            .find(|f| f.supports(&card.manifest.kind, &card.manifest.name))
            .map(|f| f.as_ref())
    }

    pub fn load_from_dir(&mut self, dir: &Path) -> common::error::Result<usize> {
        if !dir.exists() {
            warn!(path = ?dir, "capabilities directory does not exist");
            return Ok(0);
        }

        let mut count = 0;
        for entry in
            std::fs::read_dir(dir).map_err(|e| common::error::ConusAiError::Tool(e.to_string()))?
        {
            let entry = entry.map_err(|e| common::error::ConusAiError::Tool(e.to_string()))?;
            let cap_dir = entry.path();
            if !cap_dir.is_dir() { continue; }
            let manifest_path = cap_dir.join("capability.toml");
            if !manifest_path.exists() { continue; }

            let state_enabled = read_state_enabled(&cap_dir);

            match ToolManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let mut card = CapabilityCard::new(manifest, cap_dir);
                    card.enabled = state_enabled;
                    if let Some(factory) = self.factory_for(&card) {
                        match factory.create(card.clone()) {
                            Ok(provider) => card.provider = Some(provider),
                            Err(e) => {
                                warn!(name = %card.manifest.name, error = %e, "factory failed");
                                card.last_error = Some(e.to_string());
                            }
                        }
                    }
                    self.cards.insert(card.manifest.name.clone(), card);
                    count += 1;
                }
                Err(e) => warn!(path = ?manifest_path, error = %e, "failed to load manifest"),
            }
        }
        Ok(count)
    }
}

fn read_state_enabled(cap_dir: &Path) -> bool {
    let state_path = cap_dir.join("state.json");
    if !state_path.exists() { return true; }
    let Ok(raw) = std::fs::read_to_string(&state_path) else { return true };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return true };
    v["enabled"].as_bool().unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::manifest::{ToolKind, ToolManifest};

    fn make_card(name: &str, tags: Vec<String>) -> CapabilityCard {
        let manifest = ToolManifest {
            name: name.into(),
            version: "0.1.0".into(),
            description: "test".into(),
            kind: ToolKind::Chain,
            tools: vec![],
            config: serde_json::Value::Null,
            tags,
            chain: None,
        };
        CapabilityCard::new(manifest, std::path::PathBuf::from("/tmp"))
    }

    #[test]
    fn test_register_and_get() {
        let mut r = ToolRegistry::new();
        r.register(make_card("my-tool", vec!["finance".into()]));
        assert_eq!(r.len(), 1);
        assert!(r.get("my-tool").is_some());
        assert!(r.get("unknown").is_none());
    }

    #[test]
    fn test_unregister() {
        let mut r = ToolRegistry::new();
        r.register(make_card("tool-a", vec![]));
        assert!(r.unregister("tool-a"));
        assert!(!r.unregister("tool-a"));
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_set_enabled() {
        let mut r = ToolRegistry::new();
        r.register(make_card("tool-b", vec![]));
        assert!(r.set_enabled("tool-b", false));
        assert!(!r.get("tool-b").unwrap().enabled);
        assert_eq!(r.all_enabled().count(), 0);
        assert!(r.set_enabled("tool-b", true));
        assert_eq!(r.all_enabled().count(), 1);
    }

    #[test]
    fn test_search_by_tag() {
        let mut r = ToolRegistry::new();
        r.register(make_card("a", vec!["finance".into()]));
        r.register(make_card("b", vec!["storage".into()]));
        r.register(make_card("c", vec!["finance".into(), "ocr".into()]));
        assert_eq!(r.search_by_tag("finance").len(), 2);
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let mut r = ToolRegistry::new();
        let result = r.load_from_dir(std::path::Path::new("/tmp/nonexistent-xyzabc"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
