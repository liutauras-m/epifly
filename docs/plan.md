# ConusAI Platform Improvement Plan

Date: 2026-05-04
Owner: Backend platform team
Status: Ready to execute

## Objective

Apply community-aligned naming and extensibility refinements with zero feature creep and no runtime behavior regressions.

## Scope

In scope:
- Naming refactor to Capability-first terminology as a direct canonical rename.
- PromptTemplate extraction from agent-core into common for cross-crate reuse.
- Injectable context truncation strategy for workspace context assembly.
- Rig 0.36 streaming alignment cleanup in Anthropic provider path.
- Architecture/docs refresh to reflect final public API names and compatibility policy.

Out of scope:
- New product features.
- Askama to Next.js switch in v0.x.
- Changes to multitenancy/auth/rate-limit behavior.

## Current State Review (Validated Against Workspace)

Confirmed in codebase:
- Strong trait boundaries already exist for LLM, tools/capabilities, memory stores, and admin orchestration.
- Naming is partially updated today (for example CapabilitySummary exists, but RegisteredTool* and ToolProvider* remain).
- Context truncation in ContextBuilder is currently hard-coded as oldest-first and max_chars is passed by caller.
- ResolvedTenant already derives Clone.
- Anthropic streaming currently wraps complete() as a single chunk and includes a TODO to switch to native streaming helpers.

Decision:
- Keep Askama UI for v0.x. Treat Next.js app as optional future frontend that can consume existing API/SSE endpoints.

## Plan

### Phase 0 - Baseline and Safety

Goals:
- Freeze public behavior before refactor.
- Ensure deterministic rollback path.

Tasks:
- Run baseline checks for agent-core and agent-gateway.
- Snapshot OpenAPI and validate no endpoint/shape changes.
- Add release note: breaking rename policy and required code updates.

Acceptance criteria:
- Baseline tests/checks pass.
- No behavioral diffs in API responses for unchanged flows.

### Phase 1 - Naming Refactor (Canonical, Breaking)

Goals:
- Adopt Capability-first naming as the only supported API surface.

Primary renames:
- GeneralAgent -> Agent
- GeneralAgentBuilder -> AgentBuilder
- LlmProvider -> CompletionProvider (alias)
- ToolProvider -> CapabilityProvider
- ToolProviderFactory -> CapabilityFactory
- LlmChainTool -> PromptChainCapability
- RegisteredToolCard/ToolCard -> CapabilityCard
- RegisteredToolAdmin -> CapabilityAdmin

Execution strategy:
- Introduce new canonical names first.
- Remove old names, aliases, and deprecated re-exports in the same refactor window.
- Update internal references in one coordinated rename to avoid dual naming.

Acceptance criteria:
- New canonical names appear in crate-level exports and docs.
- Previous symbol names are fully removed from public exports.
- No runtime behavior changes.

### Phase 2 - PromptTemplate Extraction

Goals:
- Move PromptTemplate into common for shared reuse.

Tasks:
- Add module under common (for example common::prompt::template).
- Re-export from agent-core to preserve existing import paths.
- Update chain and prompt callers to canonical location.

Acceptance criteria:
- No functional changes in template rendering behavior.
- Existing agent-core consumers remain compatible.

### Phase 3 - Injectable Context Truncation

Goals:
- Replace hard-coded truncation logic with strategy trait.

Design:
- Add ContextTruncator trait with method to trim sections to max_chars.
- Provide default OldestFirstTruncator implementing current behavior.
- ContextBuilder accepts Arc<dyn ContextTruncator> with default constructor preserving current behavior.

Acceptance criteria:
- Default path yields identical output to current implementation.
- Unit tests cover at least default truncation and one alternate strategy.

### Phase 4 - Rig 0.36 Streaming Alignment

Goals:
- Reduce custom streaming glue and align with Rig helper patterns where available.

Tasks:
- Update anthropic provider streaming path to use Rig streaming helpers.
- Remove obsolete TODO and dead glue once parity is confirmed.

Acceptance criteria:
- Streaming endpoints continue emitting expected SSE chunk sequence.
- Token/finish metadata remain consistent with current contracts.

### Phase 5 - Documentation and Release Notes

Goals:
- Publish clear release and architecture updates.

Tasks:
- Update arch doc terminology to Capability-first naming.
- Add rename table old -> new and exact replacement rules.
- Note Askama v0.x UI decision and future optional frontend approach.

Acceptance criteria:
- Docs reflect actual exported symbols.
- v0.2 release notes include breaking rename section and examples.

## Estimates

- Phase 0: 20-30 min
- Phase 1: 60-90 min
- Phase 2: 25-40 min
- Phase 3: 25-40 min
- Phase 4: 20-30 min
- Phase 5: 20-30 min

Total: approximately 2.5 to 4.0 hours (AI-assisted, excluding review latency)

## Risks and Mitigations

- Risk: breaking external imports during rename.
	Mitigation: provide explicit old-to-new rename map and perform a single coordinated release with compile checks.

- Risk: subtle context behavior change during truncation refactor.
	Mitigation: golden tests capturing current output and order.

- Risk: streaming regressions.
	Mitigation: SSE contract tests and manual endpoint smoke checks.

## Definition of Done

- Capability-first canonical names are exported and documented.
- Previous names are removed from public API and documentation.
- PromptTemplate lives in common and is re-exported from agent-core.
- ContextBuilder uses pluggable truncation strategy with default parity.
- Streaming path aligned with Rig helper APIs where available.
- arch and release notes updated for v0.2.

## Recommended Execution Order (This Week)

1. Phase 1 (canonical naming, breaking rename).
2. Phase 3 (truncation trait) to unblock future RAG policy work.
3. Phase 2 (PromptTemplate extraction).
4. Phase 4 (streaming cleanup).
5. Phase 5 (docs/release finalization).
