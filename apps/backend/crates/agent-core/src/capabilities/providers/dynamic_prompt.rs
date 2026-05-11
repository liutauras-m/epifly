//! Factory for `DynamicPromptCapability` — manifest-backed prompt capabilities.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::ToolKind;
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::chains::dynamic_prompt::DynamicPromptCapability;
use crate::llm::LlmRegistry;
use std::sync::Arc;

pub struct DynamicPromptFactory {
    pub llm: Arc<LlmRegistry>,
}

impl DynamicPromptFactory {
    pub fn new(llm: Arc<LlmRegistry>) -> Self {
        Self { llm }
    }
}

impl CapabilityFactory for DynamicPromptFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::DynamicPrompt)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        let provider = DynamicPromptCapability::new(card.manifest, Arc::clone(&self.llm));
        Ok(Arc::new(provider))
    }
}
