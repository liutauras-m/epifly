use super::card::ToolCard;
use super::provider::ToolProvider;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Default)]
pub struct ToolRegistry {
    /// Metadata cards — kept for `/v1/capabilities` listing and search.
    cards: HashMap<String, ToolCard>,
    /// Executable providers — one per registered tool set.
    providers: HashMap<String, Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
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

    /// Register a raw card without a provider (legacy path used by `load_from_dir`
    /// when no provider factory is available).
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

    /// Load cards from a capability directory; uses the provider factory to
    /// instantiate providers when available.
    pub fn load_from_dir(&mut self, dir: &std::path::Path) -> common::error::Result<usize> {
        use super::{card::ToolCard, manifest::ToolManifest};

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
                    // Try to create a provider; fall back to card-only if unsupported.
                    match super::providers::provider_for(card.clone()) {
                        Ok(provider) => self.register_provider(provider),
                        Err(_) => self.register_card(card),
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
            kind: ToolKind::Pipeline,
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
