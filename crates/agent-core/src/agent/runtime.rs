use super::builder::{GeneralAgent, GeneralAgentBuilder};
use crate::capabilities::registry::CapabilityRegistry;
use crate::context::tenant::TenantContext;
use tracing::{info, instrument};

pub struct AgentRuntime {
    agent: GeneralAgent,
    registry: CapabilityRegistry,
}

impl AgentRuntime {
    pub fn new(agent: GeneralAgent, registry: CapabilityRegistry) -> Self {
        Self { agent, registry }
    }

    pub fn for_tenant(
        model: impl Into<String>,
        preamble: impl Into<String>,
        registry: CapabilityRegistry,
        tenant: TenantContext,
    ) -> Self {
        let agent = GeneralAgentBuilder::build_for_tenant(model, preamble, tenant);
        Self { agent, registry }
    }

    #[instrument(skip(self), fields(
        tenant_id = self.agent.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none"),
        capabilities = self.registry.len()
    ))]
    pub async fn run(&self, input: &str) -> common::error::Result<String> {
        info!("running agent runtime");
        self.agent.prompt(input).await
    }

    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }
}
