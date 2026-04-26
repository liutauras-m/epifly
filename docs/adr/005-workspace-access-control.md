# ADR 005: Workspace Access Control — Private by Default + Selective Sharing

**Status:** Accepted
**Date:** 2026-04-26

## Decision

Each `WorkspaceNode` carries `owner_id: String` and `shared_with: Vec<String>` (user IDs).
All store queries filter on the current `TenantContext.user_id` (mapped to `"__dev__"` sentinel when `None` in dev mode).
Sharing is explicit per node — no inheritance; sharing a folder does not share its children.

## Rationale

- Predictable: each node's ACL is self-contained and auditable.
- Reuses `TenantContext.user_id` (already populated from JWT `sub` claim in prod).
- Qdrant-native: `owner_id` and `shared_with` are keyword-indexed payload fields; the `should` / `min_should` filter expression covers both cases in one query.
- Dev mode is safe: `user_id = None` maps to `"__dev__"` so the same store logic runs in both modes.

## Consequences

- Non-owners receive `NotFound` (not `Forbidden`) when accessing a node they cannot see. This prevents existence leakage.
- Recursive operations (delete, move) must propagate access checks on every node in the subtree.
- Cross-tenant sharing is explicitly out of scope and deferred to a future ADR.
