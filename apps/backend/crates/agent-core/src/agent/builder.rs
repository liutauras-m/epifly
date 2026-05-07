use crate::agent::hooks::TracingHook;
use crate::context::tenant::TenantContext;
use crate::tools::semantic_router::SemanticCapabilityRouter;
use rig::client::ProviderClient;
use rig::client::completion::CompletionClient;
use rig::completion::Prompt;
use rig::providers::anthropic;
use std::sync::Arc;
use tracing::{info, instrument};

pub struct AgentBuilder {
    model: String,
    preamble: String,
    max_tokens: u64,
    tenant: Option<TenantContext>,
    semantic_router: Option<Arc<SemanticCapabilityRouter>>,
}

impl AgentBuilder {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            preamble: "You are a helpful AI assistant.".into(),
            max_tokens: 4096,
            tenant: None,
            semantic_router: None,
        }
    }

    pub fn preamble(mut self, p: impl Into<String>) -> Self {
        self.preamble = p.into();
        self
    }

    pub fn max_tokens(mut self, n: u64) -> Self {
        self.max_tokens = n;
        self
    }

    pub fn with_tenant(mut self, tenant: TenantContext) -> Self {
        self.tenant = Some(tenant);
        self
    }

    /// Wire a `SemanticCapabilityRouter` so the agent pre-filters tools
    /// to top-K per turn rather than sending all registered tools.
    pub fn with_semantic_router(mut self, router: Arc<SemanticCapabilityRouter>) -> Self {
        self.semantic_router = Some(router);
        self
    }

    pub fn build(self) -> Agent {
        let client = anthropic::Client::from_env();
        let max_tokens = self
            .tenant
            .as_ref()
            .map(|t| t.plan.max_tokens().min(self.max_tokens))
            .unwrap_or(self.max_tokens);

        let inner = client
            .expect("ANTHROPIC_API_KEY must be set")
            .agent(&self.model)
            .preamble(&self.preamble)
            .max_tokens(max_tokens)
            .build();

        Agent {
            inner,
            tenant: self.tenant,
            semantic_router: self.semantic_router,
            model: self.model,
            preamble: self.preamble,
            max_tokens,
        }
    }

    /// Convenience constructor that wires tenant limits automatically.
    pub fn build_for_tenant(
        model: impl Into<String>,
        preamble: impl Into<String>,
        tenant: TenantContext,
    ) -> Agent {
        Self::new(model)
            .preamble(preamble)
            .with_tenant(tenant)
            .build()
    }
}

pub struct Agent {
    inner: rig::agent::Agent<rig::providers::anthropic::completion::CompletionModel>,
    pub tenant: Option<TenantContext>,
    /// If set, tool definitions for each turn are resolved via semantic routing.
    pub semantic_router: Option<Arc<SemanticCapabilityRouter>>,
    model: String,
    preamble: String,
    max_tokens: u64,
}

impl Agent {
    #[instrument(skip(self), fields(tenant_id = self.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none")))]
    pub async fn prompt(&self, text: &str) -> common::error::Result<String> {
        info!("agent prompt");
        let hook = TracingHook::new(
            self.tenant
                .as_ref()
                .map(|t| t.tenant_id.as_str())
                .unwrap_or("none"),
            self.tenant
                .as_ref()
                .map(|t| t.plan.to_string())
                .unwrap_or_default(),
            None,
        );
        let max_turns = self
            .tenant
            .as_ref()
            .map(|t| t.plan.max_turns() as usize)
            .unwrap_or(10);

        if let Some(router) = &self.semantic_router {
            let tools = router
                .rig_tools_for_prompt(text, self.tenant.as_ref())
                .await
                .map_err(common::error::ConusAiError::Other)?;

            let client = anthropic::Client::from_env()
                .map_err(|e| common::error::ConusAiError::Other(e.into()))?;

            let routed_agent = client
                .agent(&self.model)
                .preamble(&self.preamble)
                .max_tokens(self.max_tokens)
                .tools(tools)
                .build();

            return routed_agent
                .prompt(text)
                .max_turns(max_turns)
                .with_hook(hook)
                .await
                .map_err(|e| common::error::ConusAiError::Other(e.into()));
        }

        self.inner
            .prompt(text)
            .max_turns(max_turns)
            .with_hook(hook)
            .await
            .map_err(|e| common::error::ConusAiError::Other(e.into()))
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new("claude-sonnet-4-6")
    }
}
