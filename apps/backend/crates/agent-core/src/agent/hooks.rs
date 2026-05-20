/// Rig v0.35+ hook implementations for the ConusAI agent pipeline.
///
/// Hooks implement cross-cutting concerns (tracing, permission checks) without
/// polluting handler logic. Attach via `AgentBuilder::hook(TracingHook::new(...))`.
use crate::capabilities::executor::{PlanStep, StepResult, run_plan};
use crate::capabilities::registry::CapabilityRegistry;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::CompletionModel;
use std::sync::{Arc, Mutex};
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
    /// Whether to block `plan.orchestrate` tool calls entirely.
    pub deny_plan_orchestrate: bool,
    /// Whether to block `compute.*` tool calls.
    pub deny_compute: bool,
}

impl PermissionHook {
    /// Allow all tools (Enterprise/Pro).
    pub fn allow_all() -> Self {
        Self {
            allowed_tools: vec![],
            deny_plan_orchestrate: false,
            deny_compute: false,
        }
    }

    /// Restrict to specific tool names (Free tier).
    pub fn allow(tools: Vec<String>) -> Self {
        Self {
            allowed_tools: tools,
            deny_plan_orchestrate: false,
            deny_compute: false,
        }
    }

    /// Deny recursive orchestration (depth > 1 guard) and compute tools.
    pub fn with_plan_restrictions(mut self, deny_orchestrate: bool, deny_compute: bool) -> Self {
        self.deny_plan_orchestrate = deny_orchestrate;
        self.deny_compute = deny_compute;
        self
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
        let deny_orchestrate = self.deny_plan_orchestrate;
        let deny_compute = self.deny_compute;
        let tool = tool_name.to_string();
        async move {
            // Sanitised tool name uses __ separator; cap prefix uses _ for dots.
            let cap_prefix = tool.split("__").next().unwrap_or("");

            // Block recursive plan.orchestrate (plan_orchestrate after dot→_ sanitise).
            if deny_orchestrate && (cap_prefix == "plan_orchestrate" || cap_prefix == "plan.orchestrate") {
                warn!(tool_name = %tool, "PermissionHook: recursive plan.orchestrate denied");
                return ToolCallHookAction::skip(
                    "Recursive orchestration (plan.orchestrate inside a plan) is not permitted."
                        .to_string(),
                );
            }

            // Block compute.* tools when compute is denied for this tenant.
            if deny_compute && cap_prefix.starts_with("compute") {
                warn!(tool_name = %tool, "PermissionHook: compute.* tool denied for this plan tier");
                return ToolCallHookAction::skip(format!(
                    "Tool '{}' requires a plan that includes compute capabilities.",
                    tool
                ));
            }

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

// ── OrchestrationHook ─────────────────────────────────────────────────────────

/// Hook that intercepts tool results containing a `plan_steps` array and
/// executes them via `run_plan`.
///
/// Rig 0.36's `HookAction` does not support in-place result amendment, so
/// this hook fires `run_plan` as a side effect and stores results in a shared
/// buffer (`plan_results`) that the caller can read after the agent turn via
/// `take_results()`.  SSE ordering is preserved because the hook runs inside
/// Rig's `on_tool_result` callback, which is serialised before the next
/// completion call.
#[derive(Clone)]
pub struct OrchestrationHook {
    registry: Arc<Mutex<CapabilityRegistry>>,
    llm: Arc<LlmRegistry>,
    tenant: Option<TenantContext>,
    /// Accumulated plan results from all intercepted `plan_steps` in this turn.
    plan_results: Arc<Mutex<Vec<StepResult>>>,
}

impl OrchestrationHook {
    pub fn new(
        registry: Arc<Mutex<CapabilityRegistry>>,
        llm: Arc<LlmRegistry>,
        tenant: Option<TenantContext>,
    ) -> Self {
        Self {
            registry,
            llm,
            tenant,
            plan_results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Drain and return any plan results accumulated since the last call.
    pub fn take_results(&self) -> Vec<StepResult> {
        let mut lock = self.plan_results.lock().unwrap();
        std::mem::take(&mut *lock)
    }
}

impl<M: CompletionModel> PromptHook<M> for OrchestrationHook {
    fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> impl std::future::Future<Output = HookAction> + Send {
        let tool_name = tool_name.to_string();
        let result = result.to_string();
        let registry = Arc::clone(&self.registry);
        let llm = Arc::clone(&self.llm);
        let tenant = self.tenant.clone();
        let results_buf = Arc::clone(&self.plan_results);

        async move {
            // Only intercept if the result contains plan_steps.
            let parsed: serde_json::Value = match serde_json::from_str(&result) {
                Ok(v) => v,
                Err(_) => return HookAction::cont(),
            };

            let plan_steps_raw = match parsed.get("plan_steps") {
                Some(v) if v.is_array() => v.clone(),
                _ => return HookAction::cont(),
            };

            let steps: Vec<PlanStep> = match serde_json::from_value(plan_steps_raw) {
                Ok(s) => s,
                Err(e) => {
                    warn!(tool_name = %tool_name, error = %e, "OrchestrationHook: failed to parse plan_steps");
                    return HookAction::cont();
                }
            };

            // Phase 2.3b: validate each step against the live registry.
            // Unknown capability names produce a graceful error StepResult so the
            // planner can see the failure and re-plan on the next turn, rather than
            // crashing run_plan mid-execution.
            let (valid_steps, mut pre_errors): (Vec<PlanStep>, Vec<StepResult>) = {
                let reg = registry.lock().unwrap();
                let mut valid = Vec::new();
                let mut errs = Vec::new();
                for step in steps {
                    if reg.get_provider(&step.capability).is_some() {
                        valid.push(step);
                    } else {
                        warn!(
                            capability = %step.capability,
                            tool = %step.tool,
                            "OrchestrationHook: unknown capability in plan_steps — injecting error step"
                        );
                        errs.push(StepResult {
                            step_idx: valid.len() + errs.len(),
                            capability: step.capability.clone(),
                            tool: step.tool.clone(),
                            strategy: step.strategy.clone(),
                            output: None,
                            error: Some(format!(
                                "Capability '{}' is not registered. \
                                 Choose only from the available capability catalog.",
                                step.capability
                            )),
                            duration_ms: 0,
                            tokens_in: None,
                            tokens_out: None,
                            cost_hint_class: None,
                        });
                    }
                }
                (valid, errs)
            };

            info!(
                tool_name = %tool_name,
                step_count = valid_steps.len(),
                error_count = pre_errors.len(),
                "OrchestrationHook: intercepted plan_steps, executing via run_plan"
            );

            let step_results: Vec<StepResult> = run_plan(
                valid_steps,
                Arc::clone(&registry),
                Some(Arc::clone(&llm)),
                tenant.clone(),
                None,
            )
            .await;

            // Store pre-validation errors first, then execution results.
            let mut buf = results_buf.lock().unwrap();
            buf.append(&mut pre_errors);
            buf.extend(step_results);

            HookAction::cont()
        }
    }
}
