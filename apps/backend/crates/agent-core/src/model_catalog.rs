//! Model catalog — provider capabilities, alias resolution, plan gating.
//!
//! The catalog is the single source of truth for which models exist, what they
//! support, and how alias strings (e.g. `"opus"`, `"smart"`) map to canonical
//! model IDs.  All agent routes must resolve a [`ModelSpec`] before building a
//! provider request.
//!
//! ## Resolution order
//!
//! 1. `requested` is `None` → [`ModelCatalog::default_for`] for the plan.
//! 2. `requested` is `Some` → alias lookup → canonical ID → plan gate.
//!
//! ## Alias env overrides
//!
//! Set `LLM_ALIAS_<ALIAS>=<canonical-model-id>` to remap a built-in alias at
//! runtime (same keys used by `build_llm_registry` in `state.rs`).

use crate::context::tenant::PlanTier;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ── Core types ────────────────────────────────────────────────────────────────

/// A canonical model identifier string, e.g. `"claude-opus-4-7"`.
pub type ModelId = String;

/// Which upstream provider serves a given model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    OpenRouter,
    Ollama,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::OpenAi => write!(f, "openai"),
            ProviderKind::OpenRouter => write!(f, "openrouter"),
            ProviderKind::Ollama => write!(f, "ollama"),
        }
    }
}

/// Full capability spec for one model.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: ModelId,
    pub provider: ProviderKind,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    /// Whether this model is a plan-tier default (informational; used by
    /// [`ModelCatalog::default_for`]).
    pub default_for_plan: bool,
}

/// Errors from model catalog resolution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("model '{0}' not found in catalog")]
    NotFound(String),

    #[error("model '{0}' is not available on plan '{1}'")]
    PlanGated(String, String),

    #[error("streaming is not supported by model '{0}'")]
    StreamingNotSupported(String),
}

// ── Tool routing decision ─────────────────────────────────────────────────────

/// Lightweight pre-routing decision derived from request metadata only.
///
/// Built **before** full tool-definition loading.  The `selected_tools` field
/// is populated after the semantic-router stage; it is empty in the initial
/// lightweight phase.
#[derive(Debug, Clone)]
pub struct ToolRoutingDecision {
    /// Filled after full tool-definition loading; empty in the lightweight phase.
    pub selected_tools: Vec<serde_json::Value>,
    /// True when at least one concrete reason requires tool calling.
    pub tool_required: bool,
    pub reason: Option<ToolRequirementReason>,
}

/// Why the routing decision requires tools.
///
/// **Do not add new variants without explicit approval** — the plan prohibits
/// inventing additional reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRequirementReason {
    /// `forced_capability` was set on the request.
    ForcedCapability,
    /// The task requires state mutation or retrieval outside the prompt.
    ExternalStateRequired,
    /// The request carries file attachments or workspace actions.
    AttachmentOrWorkspaceOp,
}

impl ToolRoutingDecision {
    /// Build a lightweight decision from request metadata — no tool loading.
    ///
    /// `forced_capability`: value of `req.forced_capability`.
    /// `has_attachments`:   `!req.attachment_content.is_empty()`.
    pub fn from_request(forced_capability: Option<&str>, has_attachments: bool) -> Self {
        let (tool_required, reason) = if forced_capability.is_some() {
            (true, Some(ToolRequirementReason::ForcedCapability))
        } else if has_attachments {
            (true, Some(ToolRequirementReason::AttachmentOrWorkspaceOp))
        } else {
            (false, None)
        };

        ToolRoutingDecision {
            selected_tools: vec![],
            tool_required,
            reason,
        }
    }
}

// ── Catalog trait ─────────────────────────────────────────────────────────────

pub trait ModelCatalog: Send + Sync {
    /// Resolve the allowed [`ModelSpec`] for this plan and requested model string.
    fn resolve_allowed(
        &self,
        plan: &PlanTier,
        requested: Option<&str>,
    ) -> Result<&ModelSpec, ModelError>;

    /// Return the default model for the given plan tier.
    fn default_for(&self, plan: &PlanTier) -> &ModelSpec;
}

// ── Static built-in catalog ───────────────────────────────────────────────────

/// A static catalog of known Anthropic models with built-in alias resolution.
///
/// Alias targets can be overridden via [`StaticModelCatalog::with_alias_env`].
pub struct StaticModelCatalog {
    specs: HashMap<String, ModelSpec>,
    /// Alias string → canonical model ID.
    aliases: HashMap<String, String>,
}

impl StaticModelCatalog {
    /// Build the catalog with the current known model lineup.
    pub fn new() -> Self {
        let mut specs = HashMap::new();

        for spec in [
            ModelSpec {
                id: "claude-haiku-4-5".into(),
                provider: ProviderKind::Anthropic,
                max_input_tokens: 200_000,
                max_output_tokens: 8_192,
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                default_for_plan: false,
            },
            ModelSpec {
                id: "claude-sonnet-4-6".into(),
                provider: ProviderKind::Anthropic,
                max_input_tokens: 200_000,
                max_output_tokens: 16_384,
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                default_for_plan: false,
            },
            ModelSpec {
                id: "claude-opus-4-7".into(),
                provider: ProviderKind::Anthropic,
                max_input_tokens: 200_000,
                max_output_tokens: 32_768,
                supports_tools: true,
                supports_streaming: true,
                supports_vision: true,
                default_for_plan: true,
            },
        ] {
            specs.insert(spec.id.clone(), spec);
        }

        let mut aliases = HashMap::new();
        for (alias, target) in [
            ("opus", "claude-opus-4-7"),
            ("smart", "claude-sonnet-4-6"),
            ("sonnet", "claude-sonnet-4-6"),
            ("haiku", "claude-haiku-4-5"),
            ("fast", "claude-haiku-4-5"),
            ("cheap", "claude-haiku-4-5"),
        ] {
            aliases.insert(alias.to_string(), target.to_string());
        }

        Self { specs, aliases }
    }

    /// Override alias targets from `LLM_ALIAS_<ALIAS>` environment variables.
    ///
    /// Only remaps to model IDs that exist in the catalog; unknown IDs are
    /// silently ignored so a typo in env does not silently remove an alias.
    pub fn with_alias_env(mut self) -> Self {
        let known: Vec<String> = self.aliases.keys().cloned().collect();
        for alias in known {
            let env_key = format!("LLM_ALIAS_{}", alias.to_uppercase());
            if let Ok(model_id) = std::env::var(&env_key)
                && self.specs.contains_key(&model_id)
            {
                self.aliases.insert(alias, model_id);
            }
        }
        self
    }

    /// Resolve alias to canonical model ID.
    ///
    /// Returns `(canonical_id, was_alias)`.  Used by tests and by
    /// [`ModelCatalog::resolve_allowed`] to determine whether to emit the
    /// `model_alias_used` metric.
    pub fn resolve_alias<'a>(&'a self, requested: &'a str) -> (&'a str, bool) {
        if let Some(canonical) = self.aliases.get(requested) {
            (canonical.as_str(), true)
        } else {
            (requested, false)
        }
    }
}

impl Default for StaticModelCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether the given plan tier allows access to the given model.
fn plan_allows(plan: &PlanTier, spec: &ModelSpec) -> bool {
    match plan {
        // Free: restricted to Haiku only (cost control).
        PlanTier::Free => spec.id.contains("haiku"),
        // Pro: Haiku and Sonnet; Opus is reserved for Enterprise.
        PlanTier::Pro => !spec.id.contains("opus"),
        // Enterprise: all models.
        PlanTier::Enterprise => true,
    }
}

impl ModelCatalog for StaticModelCatalog {
    fn resolve_allowed(
        &self,
        plan: &PlanTier,
        requested: Option<&str>,
    ) -> Result<&ModelSpec, ModelError> {
        let Some(req) = requested else {
            return Ok(self.default_for(plan));
        };

        let (canonical_id, was_alias) = self.resolve_alias(req);

        let spec = self
            .specs
            .get(canonical_id)
            .ok_or_else(|| ModelError::NotFound(req.to_string()))?;

        if was_alias {
            common::metrics::record_model_alias_used(req, canonical_id);
        }

        if !plan_allows(plan, spec) {
            return Err(ModelError::PlanGated(
                req.to_string(),
                format!("{plan:?}"),
            ));
        }

        Ok(spec)
    }

    fn default_for(&self, plan: &PlanTier) -> &ModelSpec {
        let id = match plan {
            PlanTier::Free => "claude-haiku-4-5",
            PlanTier::Pro => "claude-sonnet-4-6",
            PlanTier::Enterprise => "claude-opus-4-7",
        };
        // All three IDs are guaranteed to be in the catalog.
        &self.specs[id]
    }
}

// ── Input token estimation ────────────────────────────────────────────────────

/// Estimate the number of input tokens for a request.
///
/// Uses a conservative character-based heuristic: 1 token ≈ 3.5 chars for
/// Latin scripts (this overestimates for CJK — deliberate, we fail closed).
///
/// The estimate counts serialized JSON lengths, which overestimates due to JSON
/// overhead.  This is intentional: we want to reject clearly-too-large payloads
/// while keeping the check computationally cheap (no tokenizer dependency).
///
/// Parameters are pre-computed lengths (not borrowed slices) so callers can
/// avoid serializing more than once.
pub fn estimate_input_tokens(
    messages_json_len: usize,
    system_chars: usize,
    tools_json_len: usize,
) -> u64 {
    let total = messages_json_len + system_chars + tools_json_len;
    (total as f64 / 3.5).ceil() as u64
}

/// True when the estimate exceeds `spec.max_input_tokens` with the 10 % safety margin.
///
/// Fail-closed: 10 % headroom means a conservative estimate that is 10 % above
/// the limit will still be accepted.  An estimate that is 11 % above will be
/// rejected.
pub fn token_estimate_exceeds_limit(estimate: u64, spec: &ModelSpec) -> bool {
    // Apply a 10 % safety margin: reject when estimate > max * 1.1.
    // Integer arithmetic: multiply estimate by 10 and compare to max * 11
    // to avoid floating-point in the hot path.
    estimate.saturating_mul(10) > spec.max_input_tokens.saturating_mul(11)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_resolves_to_canonical_spec_and_was_aliased() {
        let catalog = StaticModelCatalog::new();
        // resolve_alias is the testable proxy for the alias detection path
        // (the metric is emitted in resolve_allowed but is an OTel side effect).
        let (canonical, was_alias) = catalog.resolve_alias("opus");
        assert_eq!(canonical, "claude-opus-4-7");
        assert!(was_alias, "opus must be recognized as an alias");

        // Full resolution: Enterprise plan allows opus.
        let spec = catalog
            .resolve_allowed(&PlanTier::Enterprise, Some("opus"))
            .expect("opus alias should resolve for Enterprise");
        assert_eq!(spec.id, "claude-opus-4-7");
        assert!(spec.supports_tools);
        assert!(spec.supports_streaming);
        assert!(spec.supports_vision);
    }

    #[test]
    fn canonical_id_is_not_an_alias() {
        let catalog = StaticModelCatalog::new();
        let (canonical, was_alias) = catalog.resolve_alias("claude-opus-4-7");
        assert_eq!(canonical, "claude-opus-4-7");
        assert!(!was_alias, "canonical ID must not be treated as an alias");
    }

    #[test]
    fn canonical_id_resolves_directly() {
        let catalog = StaticModelCatalog::new();
        let spec = catalog
            .resolve_allowed(&PlanTier::Enterprise, Some("claude-sonnet-4-6"))
            .expect("canonical id should resolve");
        assert_eq!(spec.id, "claude-sonnet-4-6");
    }

    #[test]
    fn unknown_model_returns_not_found() {
        let catalog = StaticModelCatalog::new();
        let err = catalog
            .resolve_allowed(&PlanTier::Pro, Some("gpt-4o"))
            .unwrap_err();
        assert!(matches!(err, ModelError::NotFound(_)));
    }

    #[test]
    fn free_plan_gated_from_opus() {
        let catalog = StaticModelCatalog::new();
        let err = catalog
            .resolve_allowed(&PlanTier::Free, Some("claude-opus-4-7"))
            .unwrap_err();
        assert!(matches!(err, ModelError::PlanGated(_, _)));
    }

    #[test]
    fn pro_plan_gated_from_opus() {
        let catalog = StaticModelCatalog::new();
        let err = catalog
            .resolve_allowed(&PlanTier::Pro, Some("claude-opus-4-7"))
            .unwrap_err();
        assert!(matches!(err, ModelError::PlanGated(_, _)));
    }

    #[test]
    fn free_plan_default_is_haiku() {
        let catalog = StaticModelCatalog::new();
        let spec = catalog.default_for(&PlanTier::Free);
        assert_eq!(spec.id, "claude-haiku-4-5");
    }

    #[test]
    fn pro_plan_default_is_sonnet() {
        let catalog = StaticModelCatalog::new();
        let spec = catalog.default_for(&PlanTier::Pro);
        assert_eq!(spec.id, "claude-sonnet-4-6");
    }

    #[test]
    fn enterprise_plan_default_is_opus() {
        let catalog = StaticModelCatalog::new();
        let spec = catalog.default_for(&PlanTier::Enterprise);
        assert_eq!(spec.id, "claude-opus-4-7");
    }

    #[test]
    fn none_requested_returns_plan_default() {
        let catalog = StaticModelCatalog::new();
        let spec = catalog
            .resolve_allowed(&PlanTier::Pro, None)
            .expect("None should resolve to default");
        assert_eq!(spec.id, "claude-sonnet-4-6");
    }

    #[test]
    fn tool_routing_decision_forced_cap() {
        let d = ToolRoutingDecision::from_request(Some("my-cap"), false);
        assert!(d.tool_required);
        assert_eq!(d.reason, Some(ToolRequirementReason::ForcedCapability));
    }

    #[test]
    fn tool_routing_decision_attachments() {
        let d = ToolRoutingDecision::from_request(None, true);
        assert!(d.tool_required);
        assert_eq!(d.reason, Some(ToolRequirementReason::AttachmentOrWorkspaceOp));
    }

    #[test]
    fn tool_routing_decision_plain_chat() {
        let d = ToolRoutingDecision::from_request(None, false);
        assert!(!d.tool_required);
        assert_eq!(d.reason, None);
    }

    #[test]
    fn token_estimate_is_conservative() {
        // 3500 chars / 3.5 = exactly 1000 tokens.
        let est = estimate_input_tokens(3500, 0, 0);
        assert_eq!(est, 1000);
    }

    #[test]
    fn token_estimate_exceeds_limit_detects_oversize() {
        let catalog = StaticModelCatalog::new();
        let spec = &catalog.specs["claude-haiku-4-5"]; // max_input = 200_000
        // 300_000 tokens > 200_000 * 1.1 = 220_000 → must be flagged.
        assert!(token_estimate_exceeds_limit(300_000, spec));
    }

    #[test]
    fn token_estimate_within_limit_passes() {
        let catalog = StaticModelCatalog::new();
        let spec = &catalog.specs["claude-haiku-4-5"]; // max_input = 200_000
        // 100_000 tokens < 200_000 * 1.1 → must pass.
        assert!(!token_estimate_exceeds_limit(100_000, spec));
    }

    #[test]
    fn token_estimate_just_at_margin_passes() {
        let catalog = StaticModelCatalog::new();
        let spec = &catalog.specs["claude-haiku-4-5"]; // max_input = 200_000
        // Exactly at the 110% boundary (220_000) must pass (not-exceeded).
        assert!(!token_estimate_exceeds_limit(220_000, spec));
    }

    #[test]
    fn all_aliases_resolve_to_known_specs() {
        let catalog = StaticModelCatalog::new();
        for (alias, target) in &catalog.aliases {
            assert!(
                catalog.specs.contains_key(target.as_str()),
                "alias '{alias}' points to unknown spec '{target}'"
            );
        }
    }
}
