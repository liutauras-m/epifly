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
- Qdrant-native: `owner_id` and `shared_with` are keyword-indexed payload fields; the `min_should` (struct form: `{conditions, min_count}`) filter covers both cases in one query.
- Dev mode is safe: `user_id = None` maps to `"__dev__"` via `common::memory::workspace::effective_user_id` so the same store logic runs in both modes.

## Implementation reference

[`QdrantWorkspaceStore::access_filter`](../../crates/agent-core/src/memory/qdrant_workspace_store.rs):

```rust
fn access_filter(tenant_id: &str, user_id: &str, extra: Vec<Value>) -> Value {
    let mut must = vec![json!({"key": "tenant_id", "match": {"value": tenant_id}})];
    must.extend(extra);
    json!({
        "must": must,
        "min_should": {
            "conditions": [
                {"key": "owner_id",    "match": {"value": user_id}},
                {"key": "shared_with", "match": {"value": user_id}}
            ],
            "min_count": 1
        }
    })
}
```

This filter is applied by `list_accessible_children`, `search_nodes`, and the substring fallback. `get_accessible_node` performs the equivalent check in Rust on the raw payload because Qdrant's `points/{id}` GET does not accept a filter. `share_node` / `unshare_node` additionally enforce the `owner_id == caller` invariant before mutating `shared_with`.

## Consequences

- Non-owners receive `NotFound` (not `Forbidden`) when accessing a node they cannot see. This prevents existence leakage.
- Recursive operations (delete, move) must propagate access checks on every node in the subtree.
- Cross-tenant sharing is explicitly out of scope and deferred to a future ADR.
