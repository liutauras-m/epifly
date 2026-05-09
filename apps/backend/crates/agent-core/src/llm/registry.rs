use crate::context::tenant::TenantContext;
use crate::llm::error::LlmError;
use crate::llm::provider::CompletionProvider;
use crate::llm::types::LlmBinding;
use common::config::LlmConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};

/// Single source of truth for provider and model resolution.
///
/// Resolution order inside `resolve` / `resolve_binding`:
/// 1. `tenant.preferred_model` (alias or concrete model id).
/// 2. `alias_or_model` argument supplied by the caller.
/// 3. `tenant.plan.default_alias()`.
/// 4. `self.default` (from `[llm].default` in config).
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
        Self { providers, aliases, default }
    }

    /// Build a registry from the parsed `[llm]` config section.
    ///
    /// `providers_map` must contain one entry per `LlmAliasConfig.provider` value
    /// found in `config.aliases`.  Typically assembled in `main.rs` after reading
    /// env vars.
    pub fn from_config(
        config: &LlmConfig,
        providers_map: HashMap<String, Arc<dyn CompletionProvider>>,
    ) -> Result<Self, LlmError> {
        let aliases: HashMap<String, LlmBinding> = config
            .aliases
            .iter()
            .map(|(alias, cfg)| {
                (
                    alias.clone(),
                    LlmBinding {
                        provider: cfg.provider.clone(),
                        model: cfg.model.clone(),
                    },
                )
            })
            .collect();

        let default_binding = aliases
            .get(&config.default)
            .cloned()
            .ok_or_else(|| LlmError::UnknownAlias { alias: config.default.clone() })?;

        Ok(Self::new(providers_map, aliases, default_binding))
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
        // 1. Tenant preferred model
        if let (Some(_), Some(preferred)) = (tenant, tenant.and_then(|t| t.preferred_model.as_deref())) {
            if let Some(binding) = self.aliases.get(preferred) {
                return Ok(binding.clone());
            }
            // Looks like a concrete model id — bind to the default provider.
            return Ok(LlmBinding {
                provider: self.default.provider.clone(),
                model: preferred.to_string(),
            });
        }

        // 2. Caller-supplied alias or model id
        if let Some(binding) = self.aliases.get(alias_or_model) {
            return Ok(binding.clone());
        }

        // 3. Plan default alias
        if let Some(t) = tenant {
            let plan_alias = t.plan.default_alias();
            if let Some(binding) = self.aliases.get(plan_alias) {
                return Ok(binding.clone());
            }
        }

        // 4. Global registry default
        Ok(self.default.clone())
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
        registry.providers.get(&binding.provider).ok_or_else(|| {
            LlmError::UnknownAlias { alias: alias.clone() }
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
            let chunk = LlmChunk { delta: format!("mock:{}", req.model), finish_reason: Some("stop".into()) };
            Ok(Box::pin(stream::once(async move { Ok(chunk) })))
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_registry(
        providers: &[(&str, &str)], // (provider_name, model_for_alias entries)
        aliases: &[(&str, &str, &str)], // (alias, provider, model)
        default_provider: &str,
        default_model: &str,
    ) -> LlmRegistry {
        let providers_map: HashMap<String, Arc<dyn CompletionProvider>> = providers
            .iter()
            .map(|(name, _)| (name.to_string(), Arc::new(MockProvider) as Arc<dyn CompletionProvider>))
            .collect();
        let aliases_map: HashMap<String, LlmBinding> = aliases
            .iter()
            .map(|(alias, provider, model)| {
                (alias.to_string(), LlmBinding { provider: provider.to_string(), model: model.to_string() })
            })
            .collect();
        LlmRegistry::new(
            providers_map,
            aliases_map,
            LlmBinding { provider: default_provider.into(), model: default_model.into() },
        )
    }

    fn tenant(plan: PlanTier, preferred: Option<&str>) -> TenantContext {
        let mut t = TenantContext::new("t1", None::<String>, plan, "/tmp");
        t.preferred_model = preferred.map(String::from);
        t
    }

    // ── Resolution order tests ────────────────────────────────────────────────

    #[test]
    fn step1_tenant_preferred_alias_wins() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("opus", "anthropic", "claude-opus-4-7"), ("haiku", "anthropic", "claude-haiku-4-5")],
            "anthropic", "claude-haiku-4-5",
        );
        let t = tenant(PlanTier::Free, Some("opus"));
        let b = reg.resolve_binding("haiku", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-opus-4-7");
    }

    #[test]
    fn step1_tenant_preferred_concrete_model_uses_default_provider() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("haiku", "anthropic", "claude-haiku-4-5")],
            "anthropic", "claude-haiku-4-5",
        );
        let t = tenant(PlanTier::Free, Some("claude-opus-4-6"));
        let b = reg.resolve_binding("haiku", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-opus-4-6");
        assert_eq!(b.provider, "anthropic");
    }

    #[test]
    fn step2_caller_alias_used_when_no_tenant_override() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("opus", "anthropic", "claude-opus-4-7"), ("haiku", "anthropic", "claude-haiku-4-5")],
            "anthropic", "claude-haiku-4-5",
        );
        let t = tenant(PlanTier::Free, None);
        let b = reg.resolve_binding("opus", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-opus-4-7");
    }

    #[test]
    fn step3_plan_default_alias_used_when_caller_unknown() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("opus", "anthropic", "claude-opus-4-7"), ("haiku", "anthropic", "claude-haiku-4-5")],
            "anthropic", "claude-opus-4-7",
        );
        // Free plan → default_alias = "haiku"; caller passes unknown alias
        let t = tenant(PlanTier::Free, None);
        let b = reg.resolve_binding("unknown-alias", Some(&t)).unwrap();
        assert_eq!(b.model, "claude-haiku-4-5");
    }

    #[test]
    fn step4_global_default_when_nothing_else_matches() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[], // no aliases at all
            "anthropic", "claude-haiku-4-5",
        );
        let b = reg.resolve_binding("anything", None).unwrap();
        assert_eq!(b.model, "claude-haiku-4-5");
    }

    #[test]
    fn resolve_returns_err_when_provider_not_registered() {
        let reg = make_registry(
            &[("anthropic", "")],
            &[("opus", "openai", "gpt-4o")], // openai provider not registered
            "anthropic", "claude-haiku-4-5",
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
            "anthropic", "claude-haiku-4-5",
        );
        assert!(verify_llm_providers(&reg).await.is_ok());
    }

    #[tokio::test]
    async fn verify_fails_when_default_provider_missing() {
        let reg = LlmRegistry::new(
            HashMap::new(), // no providers at all
            HashMap::new(),
            LlmBinding { provider: "anthropic".into(), model: "claude-haiku-4-5".into() },
        );
        assert!(verify_llm_providers(&reg).await.is_err());
    }
}
