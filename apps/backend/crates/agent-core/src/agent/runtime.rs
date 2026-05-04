use super::builder::{GeneralAgent, GeneralAgentBuilder};
use crate::context::tenant::TenantContext;
use crate::tools::registry::ToolRegistry;
use common::error::HttpError;
use tracing::{info, instrument};

pub struct AgentRuntime {
    agent: GeneralAgent,
    registry: ToolRegistry,
}

impl AgentRuntime {
    pub fn new(agent: GeneralAgent, registry: ToolRegistry) -> Self {
        Self { agent, registry }
    }

    pub fn for_tenant(
        model: impl Into<String>,
        preamble: impl Into<String>,
        registry: ToolRegistry,
        tenant: TenantContext,
    ) -> Self {
        let agent = GeneralAgentBuilder::build_for_tenant(model, preamble, tenant);
        Self { agent, registry }
    }

    #[instrument(skip(self), fields(
        tenant_id = self.agent.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none"),
        tools = self.registry.len()
    ))]
    pub async fn run(&self, input: &str) -> common::error::Result<String> {
        info!("running agent runtime");
        self.agent.prompt(input).await
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

/// Map a Rig (or agent pipeline) error string to the most appropriate [`HttpError`] variant.
///
/// Rig error types are not guaranteed to be `'static + Send` across all releases, so we
/// perform string-pattern matching on the `Display` output rather than `downcast_ref`.
/// This is intentionally conservative: unrecognised errors fall back to `agent`.
pub fn map_rig_error(msg: impl AsRef<str>) -> HttpError {
    let m = msg.as_ref();
    let lower = m.to_lowercase();

    if lower.contains("max turns")
        || lower.contains("maxturns")
        || lower.contains("maximum number of turns")
    {
        // Agent reached its turn cap — surface as a descriptive agent error, not 500.
        HttpError::agent(format!("agent reached max-turns limit: {m}"))
    } else if lower.contains("rate limit") || lower.contains("429") {
        // Provider-side rate limit — let callers retry after a delay.
        HttpError::rate_limit(Some(60))
    } else if lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("api key")
        || lower.contains("401")
    {
        HttpError::auth(m)
    } else if lower.contains("tool")
        && (lower.contains("error") || lower.contains("fail") || lower.contains("not found"))
    {
        HttpError::agent(format!("tool execution failed: {m}"))
    } else {
        HttpError::agent(m)
    }
}
