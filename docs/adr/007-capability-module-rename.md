# ADR 007 — Capability Module Rename: `tools` → `capabilities`

**Status:** Accepted  
**Date:** 2026-05-10

## Context

`agent-core` exposes a `tools` module with `ToolRegistry` at its centre. The word "tool" is overloaded: it appears in LLM tool-use (function calling), Tauri commands, and the MCP protocol. In every user-facing and admin API the word "capability" is already used (`CapabilityCard`, `CapabilityProvider`, `CapabilityFactory`, `/admin/capabilities/`). The vocabulary mismatch confuses contributors and makes the Tauri glue code awkward to read.

## Decision

Rename the `tools` module to `capabilities` and rename `ToolRegistry` → `CapabilityRegistry`. No deprecated re-exports; callers are updated in the same commit. The rename is a single behaviour-preserving PR with zero logic changes, guarded by the full eval suite.

`ToolDiscovery` is renamed to `CapabilityDiscovery` for consistency.

## Consequences

- All `use crate::tools::*` becomes `use crate::capabilities::*` across `agent-core`, `agent-gateway`, `jobs`, and test files.
- `pub use tools::registry::ToolRegistry` in `lib.rs` becomes `pub use capabilities::registry::CapabilityRegistry`.
- Eval suite (`cargo run -p evals`) must produce byte-identical report before merge.
- Any downstream crates outside this workspace that import `ToolRegistry` must be updated by their owners.
