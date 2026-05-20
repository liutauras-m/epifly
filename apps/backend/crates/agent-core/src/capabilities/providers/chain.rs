//! Chain-kind capability factory — creates `PromptChainCapability` from manifests
//! that have a `[chain]` block.
//!
//! All model calls go through `LlmRegistry` — never through `rig::providers::*::Client`
//! directly. The build.rs guard enforces this invariant at compile time.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::ToolKind;
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::chains::llm_chain::PromptChainCapability;
use crate::llm::LlmRegistry;
use std::sync::Arc;

/// Factory for `ToolKind::Chain` — creates a `PromptChainCapability` from any
/// chain manifest that has a `[chain]` block. Manifests without a `[chain]` block
/// are rejected: all domain logic must be expressed in TOML prompts, not bespoke Rust.
pub struct ChainFactory {
    pub llm: Arc<LlmRegistry>,
}

impl ChainFactory {
    pub fn new(llm: Arc<LlmRegistry>) -> Self {
        Self { llm }
    }
}

impl CapabilityFactory for ChainFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Chain)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        if card.manifest.chain.is_none() {
            anyhow::bail!(
                "ChainFactory: manifest '{}' has kind=chain but is missing the [chain] block. \
                 Add [chain] with model, prompt_template, etc. or change kind to an appropriate type.",
                card.manifest.name
            );
        }
        Ok(Arc::new(PromptChainCapability::new(
            card.manifest,
            Arc::clone(&self.llm),
        )?))
    }
}
