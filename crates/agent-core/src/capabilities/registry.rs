use super::card::CapabilityCard;
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Default)]
pub struct CapabilityRegistry {
    cards: HashMap<String, CapabilityCard>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, card: CapabilityCard) {
        info!(name = %card.manifest.name, kind = ?card.manifest.kind, "registering capability");
        self.cards.insert(card.manifest.name.clone(), card);
    }

    pub fn get(&self, name: &str) -> Option<&CapabilityCard> {
        self.cards.get(name)
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&CapabilityCard> {
        self.cards
            .values()
            .filter(|c| c.manifest.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &CapabilityCard> {
        self.cards.values()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn load_from_dir(&mut self, dir: &std::path::Path) -> common::error::Result<usize> {
        use super::{card::CapabilityCard, manifest::CapabilityManifest};

        if !dir.exists() {
            warn!(path = ?dir, "capabilities directory does not exist");
            return Ok(0);
        }

        let mut count = 0;
        for entry in std::fs::read_dir(dir)
            .map_err(|e| common::error::ConusAiError::Capability(e.to_string()))?
        {
            let entry =
                entry.map_err(|e| common::error::ConusAiError::Capability(e.to_string()))?;
            let cap_dir = entry.path();
            if !cap_dir.is_dir() {
                continue;
            }
            let manifest_path = cap_dir.join("capability.yaml");
            if !manifest_path.exists() {
                continue;
            }
            match CapabilityManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let card = CapabilityCard::new(manifest, cap_dir);
                    self.register(card);
                    count += 1;
                }
                Err(e) => warn!(path = ?manifest_path, error = %e, "failed to load capability"),
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::manifest::{CapabilityKind, CapabilityManifest};

    fn make_card(name: &str, tags: Vec<String>) -> CapabilityCard {
        let manifest = CapabilityManifest {
            name: name.into(),
            version: "0.1.0".into(),
            description: "test".into(),
            kind: CapabilityKind::Pipeline,
            tools: vec![],
            config: serde_json::Value::Null,
            tags,
        };
        CapabilityCard::new(manifest, std::path::PathBuf::from("/tmp"))
    }

    #[test]
    fn test_register_and_get() {
        let mut r = CapabilityRegistry::new();
        r.register(make_card("invoice-processing", vec!["finance".into()]));
        assert_eq!(r.len(), 1);
        assert!(r.get("invoice-processing").is_some());
        assert!(r.get("unknown").is_none());
    }

    #[test]
    fn test_search_by_tag() {
        let mut r = CapabilityRegistry::new();
        r.register(make_card("a", vec!["finance".into()]));
        r.register(make_card("b", vec!["storage".into()]));
        r.register(make_card("c", vec!["finance".into(), "ocr".into()]));
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
        let mut r = CapabilityRegistry::new();
        let result = r.load_from_dir(std::path::Path::new("/tmp/nonexistent-xyzabc"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
