/// Rig v0.35+ hook implementations for the ConusAI agent pipeline.
///
/// Hooks implement cross-cutting concerns (tracing, permission checks) without
/// polluting handler logic. Attach via `AgentBuilder::hook(TracingHook::new(...))`.
use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::CompletionModel;
use tracing::{info, warn};

/// Emits OpenTelemetry-compatible tracing events for every agent turn and tool call.
/// Attach this to every agent build so cross-cutting observability is guaranteed.
#[derive(Clone)]
pub struct TracingHook {
    pub tenant_id: String,
    pub plan: String,
    pub thread_id: Option<String>,
}

impl TracingHook {
    pub fn new(
        tenant_id: impl Into<String>,
        plan: impl Into<String>,
        thread_id: Option<String>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            plan: plan.into(),
            thread_id,
        }
    }
}

impl<M: CompletionModel> PromptHook<M> for TracingHook {
    fn on_completion_call(
        &self,
        _prompt: &rig::message::Message,
        _history: &[rig::message::Message],
    ) -> impl std::future::Future<Output = HookAction> + Send {
        let tenant_id = self.tenant_id.clone();
        let plan = self.plan.clone();
        let thread_id = self.thread_id.clone();
        async move {
            info!(
                tenant_id = %tenant_id,
                plan = %plan,
                thread_id = ?thread_id,
                "rig: on_completion_call"
            );
            HookAction::cont()
        }
    }

    fn on_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
    ) -> impl std::future::Future<Output = ToolCallHookAction> + Send {
        let tenant_id = self.tenant_id.clone();
        let tool_name = tool_name.to_string();
        let internal_id = internal_call_id.to_string();
        let args = args.to_string();
        async move {
            info!(
                tenant_id = %tenant_id,
                tool_name = %tool_name,
                call_id = ?tool_call_id,
                internal_call_id = %internal_id,
                args = %args,
                "rig: on_tool_call"
            );
            ToolCallHookAction::cont()
        }
    }

    fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> impl std::future::Future<Output = HookAction> + Send {
        let tool_name = tool_name.to_string();
        let result_len = result.len();
        async move {
            info!(tool_name = %tool_name, result_bytes = result_len, "rig: on_tool_result");
            HookAction::cont()
        }
    }
}

/// Permission hook: rejects tool calls not allowed for the current plan tier.
/// Uses `ToolCallHookAction::Skip { reason }` so the LLM receives a graceful denial.
#[derive(Clone)]
pub struct PermissionHook {
    pub allowed_tools: Vec<String>,
}

impl PermissionHook {
    /// Allow all tools (Enterprise/Pro).
    pub fn allow_all() -> Self {
        Self { allowed_tools: vec![] }
    }

    /// Restrict to specific tool names (Free tier).
    pub fn allow(tools: Vec<String>) -> Self {
        Self { allowed_tools: tools }
    }
}

impl<M: CompletionModel> PromptHook<M> for PermissionHook {
    fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
    ) -> impl std::future::Future<Output = ToolCallHookAction> + Send {
        let allowed = self.allowed_tools.clone();
        let tool = tool_name.to_string();
        async move {
            // Empty allow list = allow all
            if allowed.is_empty() || allowed.iter().any(|t| t == &tool) {
                ToolCallHookAction::cont()
            } else {
                warn!(tool_name = %tool, "PermissionHook: tool not allowed for this plan tier");
                ToolCallHookAction::skip(format!(
                    "Tool '{}' is not available on your current plan.",
                    tool
                ))
            }
        }
    }
}
