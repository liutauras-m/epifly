use super::card::ToolCard;
use super::provider::{ToolProvider, ToolProviderFactory};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Default)]
pub struct ToolRegistry {
    /// Metadata cards — kept for `/v1/capabilities` listing and search.
    cards: HashMap<String, ToolCard>,
    /// Executable providers — one per registered tool set.
    providers: HashMap<String, Arc<dyn ToolProvider>>,
    /// Provider factories — consulted when loading capabilities from disk.
    factories: Vec<Box<dyn ToolProviderFactory>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a registry pre-seeded with the four built-in factories
    /// (`Mcp`, `Wasm`, `Chain`, `Native`).  Use this as the starting point
    /// whenever you're about to call `load_from_dir`.
    pub fn with_default_factories() -> Self {
        use super::providers::{
            builtin::BuiltinFactory, chain::ChainFactory, mcp::McpFactory, wasm::WasmFactory,
        };
        let mut r = Self::new();
        r.register_factory(McpFactory);
        r.register_factory(WasmFactory);
        r.register_factory(ChainFactory);
        r.register_factory(BuiltinFactory);
        r
    }

    /// Convenience: `with_default_factories()` + pre-register the builtin tool card
    /// so native tools (`read_file`, `write_file`, etc.) are immediately available
    /// without a YAML capability file.
    pub fn with_builtin() -> Self {
        let mut r = Self::with_default_factories();
        let card = crate::tools::builtin_tool_card();
        r.register_card(card);
        r
    }

    /// Register a factory.  Factories are consulted in registration order;
    /// the first match wins.
    pub fn register_factory(&mut self, factory: impl ToolProviderFactory) {
        self.factories.push(Box::new(factory));
    }

    /// Register a provider; the card is derived from its manifest.
    pub fn register_provider(&mut self, provider: Arc<dyn ToolProvider>) {
        let manifest = provider.manifest();
        info!(name = %manifest.name, kind = ?manifest.kind, "registering tool provider");
        let card = ToolCard::new(manifest.clone(), std::path::PathBuf::from("."));
        let name = manifest.name.clone();
        self.cards.insert(name.clone(), card);
        self.providers.insert(name, provider);
    }

    /// Register a raw card without a provider (fallback when no factory matches).
    pub fn register_card(&mut self, card: ToolCard) {
        info!(name = %card.manifest.name, kind = ?card.manifest.kind, "registering tool card");
        self.cards.insert(card.manifest.name.clone(), card);
    }

    pub fn get(&self, name: &str) -> Option<&ToolCard> {
        self.cards.get(name)
    }

    pub fn get_provider(&self, name: &str) -> Option<&Arc<dyn ToolProvider>> {
        self.providers.get(name)
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&ToolCard> {
        self.cards
            .values()
            .filter(|c| c.manifest.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &ToolCard> {
        self.cards.values()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Find the first factory that supports this card's kind + name.
    fn factory_for(&self, card: &ToolCard) -> Option<&dyn ToolProviderFactory> {
        self.factories
            .iter()
            .find(|f| f.supports(&card.manifest.kind, &card.manifest.name))
            .map(|f| f.as_ref())
    }

    /// Load cards from a capability directory.  Uses the registered factories to
    /// instantiate providers; falls back to card-only for unrecognised kinds.
    pub fn load_from_dir(&mut self, dir: &std::path::Path) -> common::error::Result<usize> {
        use super::manifest::ToolManifest;

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
            if !cap_dir.is_dir() {
                continue;
            }
            let manifest_path = cap_dir.join("capability.yaml");
            if !manifest_path.exists() {
                continue;
            }
            match ToolManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let card = ToolCard::new(manifest, cap_dir);
                    // Try factory first, then legacy provider_for, then card-only.
                    if let Some(factory) = self.factory_for(&card) {
                        match factory.create(card.clone()) {
                            Ok(provider) => self.register_provider(provider),
                            Err(e) => {
                                warn!(name = %card.manifest.name, error = %e, "factory failed; registering card only");
                                self.register_card(card);
                            }
                        }
                    } else {
                        match super::providers::provider_for(card.clone()) {
                            Ok(provider) => self.register_provider(provider),
                            Err(_) => self.register_card(card),
                        }
                    }
                    count += 1;
                }
                Err(e) => warn!(path = ?manifest_path, error = %e, "failed to load tool manifest"),
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::manifest::{ToolKind, ToolManifest};

    fn make_card(name: &str, tags: Vec<String>) -> ToolCard {
        let manifest = ToolManifest {
            name: name.into(),
            version: "0.1.0".into(),
            description: "test".into(),
            kind: ToolKind::Chain,
            tools: vec![],
            config: serde_json::Value::Null,
            tags,
        };
        ToolCard::new(manifest, std::path::PathBuf::from("/tmp"))
    }

    #[test]
    fn test_register_and_get() {
        let mut r = ToolRegistry::new();
        r.register_card(make_card("invoice-processing", vec!["finance".into()]));
        assert_eq!(r.len(), 1);
        assert!(r.get("invoice-processing").is_some());
        assert!(r.get("unknown").is_none());
    }

    #[test]
    fn test_search_by_tag() {
        let mut r = ToolRegistry::new();
        r.register_card(make_card("a", vec!["finance".into()]));
        r.register_card(make_card("b", vec!["storage".into()]));
        r.register_card(make_card("c", vec!["finance".into(), "ocr".into()]));
        let hits = r.search_by_tag("finance");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_manifest_embedding_text() {
        let card = make_card("invoice-processing", vec!["finance".into()]);
        let text = card.manifest.embedding_text();
        assert!(text.contains("invoice-processing"));
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let mut r = ToolRegistry::new();
        let result = r.load_from_dir(std::path::Path::new("/tmp/nonexistent-xyzabc"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
