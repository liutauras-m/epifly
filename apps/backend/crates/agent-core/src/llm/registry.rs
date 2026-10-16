use crate::context::tenant::{PlanTier, TenantContext};
use crate::llm::error::LlmError;
use crate::llm::provider::CompletionProvider;
use crate::llm::types::LlmBinding;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};

/// Single source of truth for provider and model resolution.
///
/// Resolution order inside `resolve` / `resolve_binding` (server-defined
/// callers such as chain manifests):
/// 1. `alias_or_model` when it names an alias.
/// 2. `tenant.plan.default_alias()`.
/// 3. `self.default`.
///
/// Client-facing turns go through [`LlmRegistry::select_for_plan`] instead,
/// which enforces the plan's model allowlist.
pub struct LlmRegistry {
    providers: HashMap<String, Arc<dyn CompletionProvider>>,
    aliases: HashMap<String, LlmBinding>,
    default: LlmBinding,
}

impl LlmRegistry {
    pub fn new(
        providers: HashMap<String, Arc<dyn CompletionProvider>>,
        aliases: HashMap<String, LlmBinding>,
        default: LlmBinding,
    ) -> Self {
        Self {
            providers,
            aliases,
            default,
        }
    }

    // ── Resolution helpers ────────────────────────────────────────────────────

    /// Resolve the `LlmBinding` (provider name + concrete model id) according to
    /// the four-step order described on the struct.
    #[instrument(skip(self, tenant), fields(input = %alias_or_model))]
    pub fn resolve_binding(
        &self,
        alias_or_model: &str,
        tenant: Option<&TenantContext>,
    ) -> Result<LlmBinding, LlmError> {
        // 1. Caller-supplied alias
        if let Some(binding) = self.aliases.get(alias_or_model) {
            return Ok(binding.clone());
        }

        // 2. Plan default alias
        if let Some(t) = tenant {
            let plan_alias = t.plan.default_alias();
            if let Some(binding) = self.aliases.get(plan_alias) {
                return Ok(binding.clone());
            }
        }

        // 3. Global registry default
        Ok(self.default.clone())
    }

    /// Choose the model for a client-facing turn. The client may name an alias
    /// (or the concrete model id an alias resolves to) from the plan's allowlist;
    /// with no request the plan default applies. Anything else is refused.
    pub fn select_for_plan(
        &self,
        plan: &PlanTier,
        requested: Option<&str>,
    ) -> Result<LlmBinding, LlmError> {
        let allowed = plan.allowed_aliases();
        let alias = match requested.map(str::trim).filter(|r| !r.is_empty()) {
            None => plan.default_alias(),
            Some(r) => allowed
                .iter()
                .copied()
                .find(|a| *a == r || self.aliases.get(*a).is_some_and(|b| b.model == r))
                .ok_or_else(|| LlmError::ModelNotAllowed {
                    requested: r.to_string(),
                    plan: plan.to_string(),
                })?,
        };
        self.aliases
            .get(alias)
            .cloned()
            .ok_or_else(|| LlmError::UnknownAlias {
                alias: alias.to_string(),
            })
    }

    /// The provider registered under `binding.provider`.
    pub fn provider_for(
        &self,
        binding: &LlmBinding,
    ) -> Result<Arc<dyn CompletionProvider>, LlmError> {
        self.providers
            .get(&binding.provider)
            .cloned()
            .ok_or_else(|| LlmError::UnknownProvider(binding.provider.clone()))
    }

    /// Resolve to the `Arc<dyn CompletionProvider>` for the resolved binding.
    pub fn resolve(
        &self,
        alias_or_model: &str,
        tenant: Option<&TenantContext>,
    ) -> Result<Arc<dyn CompletionProvider>, LlmError> {
        let binding = self.resolve_binding(alias_or_model, tenant)?;
        self.providers
            .get(&binding.provider)
            .cloned()
            .ok_or_else(|| LlmError::UnknownProvider(binding.provider.clone()))
    }
}

// ── Startup verification ──────────────────────────────────────────────────────

/// Config-only validation run at startup.
///
/// Checks that every alias resolves to a registered provider and that the
/// default binding is reachable.  Does NOT make any outbound network calls.
pub async fn verify_llm_providers(registry: &LlmRegistry) -> Result<(), LlmError> {
    // Verify default
    registry
        .providers
        .get(&registry.default.provider)
        .ok_or_else(|| LlmError::UnknownProvider(registry.default.provider.clone()))?;

    // Verify every alias
    for (alias, binding) in &registry.aliases {
        registry
            .providers
            .get(&binding.provider)
            .ok_or_else(|| LlmError::UnknownAlias {
                alias: alias.clone(),
            })?;
        info!(alias, provider = %binding.provider, model = %binding.model, "LLM alias verified");
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::tenant::{PlanTier, TenantContext};
    use crate::llm::error::LlmError;
    use crate::llm::provider::CompletionProvider;
    use crate::llm::types::{LlmChunk, LlmRequest, LlmResponse, LlmStream};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    // ── Mock provider ─────────────────────────────────────────────────────────

    struct MockProvider;

    #[async_trait]
    impl CompletionProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse {
                content: format!("mock:{}", req.model),
                usage: None,
                finish_reason: None,
            })
        }

        async fn stream(&self, req: LlmRequest) -> Result<LlmStream, LlmError> {
            use futures::stream;
            let chunk = LlmChunk {
                delta: format!("mock:{}", req.model),
                finish_reason: Some("stop".into()),
            };
            Ok(Box::pin(stream::once(async move { Ok(chunk) })))
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_registry(
        providers: &[(&str, &str)],     // (provider_name, model_for_alias entries)
        aliases: &[(&str, &str, &str)], // (alias, provider, model)
        default_provider: &str,
        default_model: &str,
    ) -> LlmRegistry {
        let providers_map: HashMap<String, Arc<dyn CompletionProvider>> = providers
            .iter()
            .map(|(name, _)| {
                (
                    name.to_string(),
                    Arc::new(MockProvider) as Arc<dyn CompletionProvider>,
                )
            })
            .collect();
        let aliases_map: HashMap<String, LlmBinding> = aliases
            .iter()
            .map(|(alias, provider, model)| {
                (
                    alias.to_string(),
                    LlmBinding {
                        provider: provider.to_string(),
                        model: model.to_string(),
                    },
                )
            })
            .collect();
        LlmRegistry::new(
            providers_map,
            aliases_map,
            LlmBinding {
                provider: default_provider.into(),
                model: default_model.into(),
            },
        )
    }

    fn tenant(plan: PlanTier) -> TenantContext {
        TenantContext::new("t1", None::<String>, plan)
    }

    fn plan_registry() -> LlmRegistry {
        make_registry(
            &[("anthropic", "")],
            &[
                ("opus", "anthropic", "claude-opus-4-7"),
                ("smart", "anthropic", "claude-sonnet-4-6"),
                ("haiku", "anthropic", "claude-haiku-4-5"),
                ("fast", "anthropic", "claude-haiku-4-5"),
                ("cheap", "anthropic", "claude-haiku-4-5"),
            ],
            "anthropic",
            "claude-haiku-4-5",
        )
    }

    #[test]
    fn free_plan_cannot_select_opus() {
        let reg = plan_registry();
        assert!(matches!(
            reg.select_for_plan(&PlanTier::Free, Some("opus")),
            Err(LlmError::ModelNotAllowed { .. })
        ));
        assert!(matches!(
            reg.select_for_plan(&PlanTier::Free, Some("claude-opus-4-7")),
            Err(LlmError::ModelNotAllowed { .. })
        ));
    }

    #[test]
    fn plan_default_applies_without_request() {
        let reg = plan_registry();
        assert_eq!(
            reg.select_for_plan(&PlanTier::Free, None).unwrap().model,
            "claude-haiku-4-5"
        );
        assert_eq!(
            reg.select_for_plan(&PlanTier::Pro, None).unwrap().model,
            "claude-sonnet-4-6"
        );
        assert_eq!(
            reg.select_for_plan(&PlanTier::Enterprise, None)
                .unwrap()
                .model,
            "claude-opus-4-7"
        );
    }

    #[test]
    fn allowed_alias_or_its_model_id_is_accepted() {
        let reg = plan_registry();
        assert_eq!(
            reg.select_for_plan(&PlanTier::Enterprise, Some("opus"))
                .unwrap()
                .model,
            "claude-opus-4-7"
        );
        assert_eq!(
            reg.select_for_plan(&PlanTier::Pro, Some("claude-sonnet-4-6"))
                .unwrap()
                .model,
            "claude-sonnet-4-6"
        );
        assert!(reg.select_for_plan(&PlanTier::Pro, Some("gpt-9")).is_err());
    }

    #[test]
    fn step2_caller_alias_used_when_no_tenant_override() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[
                ("opus", "anthropic", "claude-opus-4-7"),
                ("haiku", "anthropic", "claude-haiku-4-5"),
            ],
            "anthropic",
            "claude-haiku-4-5",
        );
        let t = tenant(PlanTier::Free);
        let b = reg.resolve_binding("opus", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-opus-4-7");
    }

    #[test]
    fn step3_plan_default_alias_used_when_caller_unknown() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[
                ("opus", "anthropic", "claude-opus-4-7"),
                ("haiku", "anthropic", "claude-haiku-4-5"),
            ],
            "anthropic",
            "claude-opus-4-7",
        );
        // Free plan → default_alias = "haiku"; caller passes unknown alias
        let t = tenant(PlanTier::Free);
        let b = reg.resolve_binding("unknown-alias", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-haiku-4-5");
    }

    #[test]
    fn step4_global_default_when_nothing_else_matches() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[], // no aliases at all
            "anthropic",
            "claude-haiku-4-5",
        );
        let b = reg.resolve_binding("anything", None).unwrap();
        assert_eq!(b.model, "claude-haiku-4-5");
    }

    #[test]
    fn resolve_returns_err_when_provider_not_registered() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("opus", "openai", "gpt-4o")], // openai provider not registered
            "anthropic",
            "claude-haiku-4-5",
        );
        let result = reg.resolve("opus", None);
        assert!(matches!(result, Err(LlmError::UnknownProvider(_))));
    }

    // ── verify_llm_providers tests ────────────────────────────────────────────

    #[tokio::test]
    async fn verify_ok_when_all_providers_registered() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("haiku", "anthropic", "claude-haiku-4-5")],
            "anthropic",
            "claude-haiku-4-5",
        );
        assert!(verify_llm_providers(&reg).await.is_ok());
    }

    #[tokio::test]
    async fn verify_fails_when_default_provider_missing() {
        let reg = LlmRegistry::new(
            HashMap::new(), // no providers at all
            HashMap::new(),
            LlmBinding {
                provider: "anthropic".into(),
                model: "claude-haiku-4-5".into(),
            },
        );
        assert!(verify_llm_providers(&reg).await.is_err());
    }
}
