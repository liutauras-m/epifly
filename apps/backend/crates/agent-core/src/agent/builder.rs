use crate::agent::hooks::TracingHook;
use crate::context::tenant::TenantContext;
use rig::client::ProviderClient;
use rig::client::completion::CompletionClient;
use rig::completion::Prompt;
use rig::providers::anthropic;
use tracing::{info, instrument};

pub struct AgentBuilder {
    model: String,
    preamble: String,
    max_tokens: u64,
    tenant: Option<TenantContext>,
}

impl AgentBuilder {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            preamble: "You are a helpful AI assistant.".into(),
            max_tokens: 4096,
            tenant: None,
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
}

impl Agent {
    #[instrument(skip(self), fields(tenant_id = self.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none")))]
    pub async fn prompt(&self, text: &str) -> common::error::Result<String> {
        info!("agent prompt");
        let hook = TracingHook::new(
            self.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none"),
            self.tenant.as_ref().map(|t| t.plan.to_string()).unwrap_or_default(),
            None,
        );
        let max_turns = self
            .tenant
            .as_ref()
            .map(|t| t.plan.max_turns() as usize)
            .unwrap_or(10);
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
