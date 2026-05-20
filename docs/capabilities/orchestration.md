# Capability Orchestration

> How the agent plans and executes multi-step capability pipelines.

---

## Overview

The agent loop delegates all tool execution to the `SemanticCapabilityRouter`. For single-step requests, the router selects the best-matching capability and invokes it. For multi-step plans, the agent emits a `plan_orchestrate` tool call containing a `plan_steps` array, which is intercepted by `OrchestrationHook`.

```
User message
  └─ SemanticCapabilityRouter.select()  → top-K matching capabilities
  └─ Agent sends tools to LLM
  └─ LLM chooses tool (may choose plan_orchestrate)
  └─ OrchestrationHook.on_tool_result()  → executes plan_steps
  └─ StepResults stored in hook buffer
  └─ Caller reads results via take_results()
```

---

## Plan Steps

A `plan_steps` array has this shape:

```json
[
  {
    "capability": "invoice-processing",
    "tool": "extract_invoice",
    "input": { "image_path": "/tmp/invoice.pdf" },
    "strategy": "single"
  },
  {
    "capability": "ocr-service",
    "tool": "extract_text",
    "input": { "image_path": "/tmp/invoice.pdf" },
    "strategy": "parallel_consensus",
    "fallback_capability": "invoice-processing",
    "fallback_tool": "extract_invoice"
  }
]
```

### Strategies

| Strategy | Description |
|---|---|
| `single` | Invoke one capability. |
| `parallel_consensus` | Invoke two capabilities concurrently; use an LLM judge to select the best result. |
| `fallback_cascade` | Try the primary; if it fails, try the fallback. Returns `{fallback: true}` if both fail. |

---

## `run_plan`

```rust
pub async fn run_plan(
    steps: Vec<PlanStep>,
    registry: Arc<Mutex<CapabilityRegistry>>,
    llm: Option<Arc<LlmRegistry>>,
    tenant: Option<TenantContext>,
) -> Vec<StepResult>
```

- The registry lock is held only for the brief lookup of `Arc<dyn CapabilityProvider>`, never across an `.await` point.
- Steps are executed sequentially. Parallel execution within a step (consensus) uses `tokio::join!`.

---

## Permissions

`PermissionHook` can restrict orchestration:

```rust
PermissionHook::default()
    .with_plan_restrictions(
        deny_orchestrate: true,  // block plan_orchestrate calls
        deny_compute: false,     // allow compute capabilities
    )
```

---

## Hook Lifecycle

`OrchestrationHook` implements Rig's `PromptHook` interface. Because Rig 0.36 `HookAction` has no `AmendToolResult` variant (see ADR-0008), plan results are stored in a buffer and retrieved after the agent turn:

```rust
let hook = OrchestrationHook::new(registry, llm, tenant);
// ... run agent turn ...
let plan_results: Vec<StepResult> = hook.take_results();
```
