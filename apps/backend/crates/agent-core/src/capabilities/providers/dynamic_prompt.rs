//! Factory for `DynamicPromptCapability` — DB-backed, versioned prompt capabilities.
//!
//! Capabilities with `kind = "dynamic_prompt"` are created by this factory.
//! The factory requires a Postgres pool; without one it rejects creation.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::ToolKind;
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::chains::dynamic_prompt::DynamicPromptCapability;
use crate::llm::LlmRegistry;
use sqlx::PgPool;
use std::sync::Arc;

pub struct DynamicPromptFactory {
    pub pool: PgPool,
    pub llm: Arc<LlmRegistry>,
}

impl DynamicPromptFactory {
    pub fn new(pool: PgPool, llm: Arc<LlmRegistry>) -> Self {
        Self { pool, llm }
    }
}

impl CapabilityFactory for DynamicPromptFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::DynamicPrompt)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        let provider =
            DynamicPromptCapability::new(card.manifest, Arc::clone(&self.llm), self.pool.clone());
        Ok(Arc::new(provider))
    }
}
