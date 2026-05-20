# ADR-0008 — OrchestrationHook: Observer Pattern over Subexecution

**Status:** Accepted  
**Date:** 2026-05-20  
**Author:** Engineering

---

## Context

The plan called for an `OrchestrationHook` that intercepts `plan_steps` arrays in tool results and executes them as capability sub-plans within the agent turn. The ideal API would amend the tool result with the execution output before the LLM sees it.

Rig 0.36's `HookAction` enum only has two variants:

```rust
pub enum HookAction {
    Continue,
    Terminate,
}
```

There is no `AmendToolResult(Value)` variant. A PR to upstream Rig to add this variant would delay the implementation by weeks.

---

## Decision

`OrchestrationHook` is implemented as an **observer** rather than a mutator:

1. `on_tool_result()` detects `plan_steps` arrays in the tool result JSON.
2. It calls `run_plan(steps, registry, llm, tenant)` to execute the sub-plan.
3. Results are stored in an `Arc<Mutex<Vec<StepResult>>>` buffer on the hook.
4. The hook returns `HookAction::Continue` — the original tool result is unchanged.
5. Callers retrieve plan results via `OrchestrationHook::take_results()` after the agent turn.

```
Agent turn
  └─ tool call → plan_orchestrate
        └─ OrchestrationHook.on_tool_result()
              ├─ parse plan_steps
              ├─ run_plan(steps) → Vec<StepResult>
              └─ store in plan_results buffer
  └─ HookAction::Continue (LLM sees unmodified tool result)

After turn: caller.take_results() → Vec<StepResult>
```

### `run_plan` design

`run_plan` accepts `Arc<Mutex<CapabilityRegistry>>` (not `&CapabilityRegistry`) to avoid holding a `MutexGuard` across `.await` points. The lock is acquired briefly per step, the `Arc<dyn CapabilityProvider>` is cloned out, then the lock is released before `provider.invoke()` is called.

Three strategies are supported:

| Strategy | Behaviour |
|---|---|
| `single` | Invoke one capability; return its result |
| `parallel_consensus` | Invoke two capabilities concurrently; use LLM to judge best result |
| `fallback_cascade` | Try primary; on error try fallback; return `{fallback: true}` if both fail |

---

## Consequences

**Positive:**
- No upstream Rig changes required; works with Rig 0.36.
- `run_plan` is independently testable without an agent loop.
- `Arc<Mutex<>>` design is `Send + Sync` across tokio tasks.

**Negative:**
- The LLM in the current turn sees the original `plan_steps` tool result, not the execution output. Plan results are only visible to the **caller** after the turn completes.
- The `parallel_consensus` LLM judge adds an extra completion call to every consensus plan step.

---

## Future work

When Rig adds `HookAction::AmendToolResult` (or equivalent), the buffer pattern can be replaced with direct result injection into the tool result stream. The `OrchestrationHook` API is designed to make this migration straightforward — only `on_tool_result` needs to change.
