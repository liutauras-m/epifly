//! Factory for `DynamicPromptCapability` — manifest-backed prompt capabilities.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::ToolKind;
use crate::capabilities::object_source::TenantObjectSource;
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::chains::dynamic_prompt::DynamicPromptCapability;
use crate::llm::LlmRegistry;
use std::sync::Arc;

pub struct DynamicPromptFactory {
    pub llm: Arc<LlmRegistry>,
    pub objects: Arc<dyn TenantObjectSource>,
}

impl DynamicPromptFactory {
    pub fn new(llm: Arc<LlmRegistry>, objects: Arc<dyn TenantObjectSource>) -> Self {
        Self { llm, objects }
    }
}

impl CapabilityFactory for DynamicPromptFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::DynamicPrompt)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        let provider = DynamicPromptCapability::new(
            card.manifest,
            Arc::clone(&self.llm),
            Arc::clone(&self.objects),
        );
        Ok(Arc::new(provider))
    }
}
