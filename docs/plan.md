# ConusAI Architecture Hardening — Implementation Plan (2026-05-24)

> **Source review:** "ConusAI Architecture Hardening Plan — Response to External Review (2026-05-24)"
> **Validated against:** live codebase at `apps/backend/crates/{agent-core,agent-gateway}` and `apps/backend/evals/` on 2026‑05‑24.
> **Compliance:** Project Instructions v0.3.2 — SRP, canonical names only, no new top‑level registries, no runtime Postgres, Rig 0.36+ hooks/streaming preserved.

---

## 0. Codebase reality check (what already exists)

| Claim in review | Actual state in repo | Verdict |
|---|---|---|
| `manifest.rs` has no `schema_hash` / `permissions` / `egress_allowlist` | Confirmed — `ToolManifest` (apps/backend/crates/agent-core/src/capabilities/manifest.rs#L146-L201) lacks all three fields. | ✅ add as optional |
| `CapabilityCard` lacks provenance | Confirmed — [card.rs](apps/backend/crates/agent-core/src/capabilities/card.rs) has no author/signature/approval. | ✅ add as optional |
| `validator.rs` has no static injection scan | Confirmed — only structural validation; see [validator.rs](apps/backend/crates/agent-core/src/capabilities/validator.rs). | ✅ add `scan_for_injection_patterns` |
| `providers/mcp.rs` + `remote_mcp.rs` lack schema‑hash pinning | Confirmed — [mcp.rs](apps/backend/crates/agent-core/src/capabilities/providers/mcp.rs), [remote_mcp.rs](apps/backend/crates/agent-core/src/capabilities/providers/remote_mcp.rs) pass through to `McpAdapter` with no hash check. | ✅ enforce on registration |
| `semantic_router.rs` should take `task_tags` | The router already exposes `cfg.tags_any` (config‑level) but `rig_tools_for_prompt` does not accept per‑call tags. See [semantic_router.rs#L453-L489](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs#L453). | ✅ add overload `rig_tools_for_prompt_with_tags` |
| Qdrant cap collection is "not tenant‑scoped" | **Partially wrong.** Capability embeddings are global by design; tenant isolation happens post‑ANN via `is_visible_to()` (semantic_router.rs#L342). The `content_embeddings` collection **already** filters by `tenant_id` (qdrant_vector.rs#L348-L354). | ⚠️ rescope — see §2 |
| `/metrics`, `/docs`, `/openapi.json` are unauthenticated | Confirmed — mounted in [main.rs#L207](apps/backend/crates/agent-gateway/src/main.rs#L207) and [routes/mod.rs#L247](apps/backend/crates/agent-gateway/src/routes/mod.rs#L247). | ✅ env‑gate |
| Evals expansion fits existing `evals/runners/` | Confirmed — currently only `generic.rs` runner. | ✅ add new suites |
| redb production risk → reject | Confirmed by Project Instructions v0.3.2 §metadata store. | ✅ reject, document |
| `TaskProfileRegistry` proposal → reject | Confirmed — existing primitives cover the case. | ✅ reject |

**Net:** four of the five P0/P1 areas land as‑is. The Qdrant work must be re‑scoped (see §2) to avoid changing the global‑capability invariant.

---

## 1. P0 — MCP & capability security hardening

**Goal:** make every capability load‑time decision (schema integrity, permissions, egress) explicit on the manifest, statically scanned, and provenance‑tagged on the card. SRP preserved by extending the modules that already own each concern.

### 1.1 `manifest.rs` — optional security fields on `ToolManifest`

File: [manifest.rs](apps/backend/crates/agent-core/src/capabilities/manifest.rs)

Add (all `#[serde(default)]`, backward compatible):

```rust
// New enum in same file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityPermission {
    ReadWorkspace,
    WriteWorkspace,
    NetworkEgress,
    InvokeLlm,
    InvokeOtherCapability,
}

// Inside ToolManifest:
#[serde(default, skip_serializing_if = "Option::is_none")]
pub schema_hash: Option<String>,        // hex-encoded blake3 of input_schema set
#[serde(default)]
pub permissions: Vec<CapabilityPermission>,
#[serde(default)]
pub egress_allowlist: Vec<String>,      // host or host:port; empty = no egress
```

**Storage choice:** hex `String` (32 bytes blake3) — keeps TOML human‑readable and matches the existing `[u8;32]` convention used by `moka` cache keys without forcing every reader to base64‑decode.

**Helper on `ToolManifest`:**

```rust
pub fn compute_schema_hash(&self) -> String {
    let mut h = blake3::Hasher::new();
    for t in &self.tools {
        h.update(t.name.as_bytes());
        h.update(b"\0");
        h.update(serde_json::to_vec(&t.input_schema).unwrap_or_default().as_slice());
        h.update(b"\n");
    }
    hex::encode(h.finalize().as_bytes())
}
```

`blake3` is already in the dependency graph (used by router cache); `hex` already in tree via `qdrant-client`. Verify with `cargo tree -p agent-core`.

### 1.2 `card.rs` — provenance on `CapabilityCard`

File: [card.rs](apps/backend/crates/agent-core/src/capabilities/card.rs)

Add a small `CapabilityProvenance` struct (kept inside the same module — provenance has no behaviour, no new abstraction):

```rust
#[derive(Clone, Debug, Default)]
pub struct CapabilityProvenance {
    pub author: Option<String>,
    pub signature_id: Option<String>,
    pub approval_status: ApprovalStatus,
    pub recorded_schema_hash: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ApprovalStatus {
    #[default]
    Unreviewed,
    Approved,
    Quarantined { reason: String },
}
```

Field added to `CapabilityCard`: `pub provenance: CapabilityProvenance`. Populated by:

- `RegisteredToolStore` loaders (filesystem → `author = file owner`, others → defaults).
- `CapabilityAdmin::create` / `update` sets `approval_status = Approved` for super‑admin actions (writes are already gated).

### 1.3 `validator.rs` — static injection scanner

File: [validator.rs](apps/backend/crates/agent-core/src/capabilities/validator.rs)

Add a free function (no new struct):

```rust
/// Returns warnings (not errors) for suspicious tokens in manifest free‑text
/// fields (description, tool descriptions, system_prompt). Errors only when
/// patterns appear in tool input_schema descriptions, which agents see verbatim.
pub fn scan_for_injection_patterns(m: &ToolManifest) -> ValidationReport;
```

Detected patterns (case‑insensitive, word‑boundary):

- `ignore (all )?(previous|prior) instructions`
- `system:\s*` / `assistant:\s*` injection inside descriptions
- `<\|.*\|>` chat‑template tokens
- bare URLs in `input_schema` field descriptions (suspicious "exfil hint")
- `data:` URIs in any description string

Hooked into:

- `CapabilityFactory::build` (the existing entry point in [providers/mod.rs](apps/backend/crates/agent-core/src/capabilities/providers/mod.rs)) — runs the scan; `errors` block registration, `warnings` are surfaced on the card as `last_error = Some("warn: …")` (does NOT disable the capability).
- `RegisteredToolAdmin::validate` — also runs the scan so the UI shows warnings before save.

### 1.4 MCP schema‑hash pinning

Files: [providers/mcp.rs](apps/backend/crates/agent-core/src/capabilities/providers/mcp.rs), [providers/remote_mcp.rs](apps/backend/crates/agent-core/src/capabilities/providers/remote_mcp.rs)

Behaviour:

1. On `McpProvider::new` / `RemoteMcpProvider::new`:
   - compute `live_hash = ToolManifest::compute_schema_hash(&manifest)`.
   - If `manifest.schema_hash` is set and differs → return `Err` ("schema drift: declared `<x>` ≠ live `<y>`; bump version or re‑approve").
   - If unset → log `info!("capability '{}' has no pinned schema_hash, recording live {}", name, live_hash)` and write `card.provenance.recorded_schema_hash = Some(live_hash)`.
2. Only `CapabilityAdmin::set_approved(name, expected_hash)` (new method, super‑admin gated) may flip `provenance.approval_status = Approved` and persist the hash back to the manifest.

This preserves SRP — providers verify; admin approves; manifest stores. No new orchestrator.

### 1.5 `semantic_router.rs` — per‑call task tag filter

File: [semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs#L453)

Add a sibling overload (do not break callers):

```rust
pub async fn rig_tools_for_prompt_with_tags(
    self: &Arc<Self>,
    query: &str,
    tenant: Option<&TenantContext>,
    task_tags: &[&str],
) -> anyhow::Result<Vec<Box<dyn ToolDyn>>>;
```

Implementation: clones `self.cfg`, replaces `tags_any` with `task_tags ∪ self.cfg.tags_any`, calls a private `select_with_cfg(cfg, query, tenant, hint)` helper. Cache key already includes config‑independent inputs; tags become part of the key via a new `tags_bytes()` mixin (parallel to `AttachmentHint::cache_bytes()`).

**Verdict on output sanitization:** stays in `ArtifactBridge` (post‑execution). No change here. Confirmed against existing artifact pipeline.

### 1.6 Tests added

- `manifest::tests::compute_schema_hash_is_stable_over_field_order`
- `manifest::tests::schema_hash_changes_on_input_schema_edit`
- `validator::tests::scan_detects_ignore_previous_instructions`
- `validator::tests::scan_flags_data_uri_in_description`
- `providers::mcp::tests::rejects_on_schema_drift_when_pinned`
- `providers::remote_mcp::tests::accepts_when_hash_matches`
- `semantic_router::tests::rig_tools_for_prompt_with_tags_intersects_cfg_tags`

---

## 2. P0 — Tenant‑scoped vector queries (re‑scoped)

**Reality:** the codebase already has correct tenant isolation for the only collection that is per‑tenant:

- `content_embeddings` — every call sets `Filter::must([Condition::matches("tenant_id", …)])` (qdrant_vector.rs#L348-L354) and every upsert writes `tenant_id` into payload (qdrant_vector.rs#L490).
- `capability_embeddings` — **global by design** per Project Instructions. Tenant scoping happens after ANN via `card.is_visible_to(tenant_id)` (semantic_router.rs#L341).

The review's wording ("router now calls the tenant‑scoped helper exclusively") would silently break the global‑capability invariant. Re‑scope to:

### 2.1 Guard rails (not a refactor)

File: [qdrant_vector.rs](apps/backend/crates/agent-core/src/store/qdrant_vector.rs)

1. **Introduce `pub struct TenantScoped<'a>(&'a QdrantVectorStore, &'a str);`** with the three content methods (`search_content`, `upsert_content`, `delete_content_by_path`). The bare methods on `QdrantVectorStore` become **crate‑private** (`pub(crate)`).
2. Public API for content vectors becomes:

```rust
impl QdrantVectorStore {
    pub fn for_tenant<'a>(&'a self, tenant_id: &'a str) -> TenantScoped<'a> { ... }
}
```

3. Every external caller (`agent-gateway/src/capabilities`, indexer jobs) goes through `for_tenant()`. Compile fails if anyone calls the raw method — that is the enforcement.

### 2.2 Capability collection — assert, don't refactor

Add a `debug_assert!` in `select_with_hint` post‑ANN filter and a **eval test** (`tenant_isolation` suite §4) that seeds two tenant‑scoped capabilities and verifies a tenant A query never returns tenant B's capability id.

### 2.3 Audit

Add `scripts/check-tenant-scoped-vector.mjs` (parallel to existing `scripts/check-cross-app-imports.mjs`) that greps for `search_content_embeddings\|upsert_content_embedding` outside `for_tenant`. Wired into `make verify`.

---

## 3. P0 — Production gating for public endpoints

File creation: `apps/backend/crates/agent-gateway/src/mw/env_gate.rs`

```rust
//! Gates a route subtree behind CONUSAI_ENV=development OR a valid admin JWT.
use axum::{extract::Request, middleware::Next, response::Response, http::StatusCode};

pub async fn env_or_admin(req: Request, next: Next) -> Result<Response, StatusCode> {
    if std::env::var("CONUSAI_ENV").as_deref() == Ok("development") {
        return Ok(next.run(req).await);
    }
    // Reuse existing super‑admin JWT extractor; fall through to 404 (not 401)
    // to avoid leaking the endpoint's existence in prod.
    if mw::admin::is_super_admin_request(&req).await {
        return Ok(next.run(req).await);
    }
    Err(StatusCode::NOT_FOUND)
}
```

Wiring:

- `mw/mod.rs` → `pub mod env_gate;`
- [main.rs#L203-L209](apps/backend/crates/agent-gateway/src/main.rs#L203) → wrap the `/metrics` route layer.
- [routes/mod.rs#L247](apps/backend/crates/agent-gateway/src/routes/mod.rs#L247) → wrap `/docs` + `/openapi.json` (split the `SwaggerUi` merge into its own `Router` first so the layer applies only to docs).

**Side effect:** the route inventory (`scripts/dump-routes.sh`) and `verify-routes-doc` make target must be updated to record the gated rows.

**Non‑goal:** `/health`, `/healthz/embeddings` stay public (liveness probes).

---

## 4. P1 — Evals expansion

All work inside `apps/backend/evals/`. No new crate.

### 4.1 New runners (`evals/src/runners/`)

- `routing.rs` — drives `SemanticCapabilityRouter::select` against a fixture of `(query, expected_top1_capability)` rows; emits per‑row score and aggregate top‑K recall.
- `tenant_isolation.rs` — seeds two tenants × two `tenant_scope`‑restricted capabilities; runs `tool_definitions` for tenant A and asserts none of tenant B's names appear. Also runs the same for `for_tenant(B).search_content` to assert no doc cross‑bleed.
- `security.rs` — feeds known injection prompts into the prompt path with a tool whose description has been tampered with (`schema_hash` mismatch) and asserts (a) registration fails, (b) at runtime the agent does not call the tool.

### 4.2 New scorers (`evals/src/scorers/`)

- `top_k_recall.rs` — for routing suite (top‑1 / top‑5 / top‑10).
- `boolean_pass.rs` — for isolation + security suites (binary outcome, fails the whole eval on any leak).

### 4.3 Fixtures

`apps/backend/evals/suites/{routing,tenant_isolation,security}/cases.toml` — each row a `[[case]]` table with `query`, `expected`, optional `tenant_id`, optional `inject`. Uses existing TOML loader pattern from `generic.rs`.

### 4.4 CI hook

`justfile` already has `just evals`; add `just evals-security` and wire into `make verify`.

---

## 5. Documentation updates

Single‑pass edits to [docs/arch.md](docs/arch.md) (audit date → 2026‑05‑24). Performed as one PR alongside the code changes.

- **§12.1 redb** — append the production‑note paragraph from the review.
- **§4.2 capabilities/** — append the security‑notes paragraph.
- **§9.1 Feature Inventory** — add the three rows.
- **§12.7 Full route surface** — annotate `/metrics`, `/docs`, `/openapi.json` as env‑gated.

No other section changes. ADR not required (additive, backward‑compatible). If reviewers disagree, add `docs/adr/0006-capability-security-hardening.md` summarising §1.

---

## 6. Out of scope (explicitly rejected from the external review)

| Proposal | Reason for rejection |
|---|---|
| Move metadata to Postgres in runtime | Forbidden by Project Instructions v0.3.2; redb is the single source of truth. Snapshot job to RustFS already planned in `apps/backend/crates/jobs/`. |
| `TaskProfileRegistry` / new control‑plane abstraction | Violates "no unnecessary abstractions". `SemanticCapabilityRouter` + `NamespaceFilter` + `task_tags` (§1.5) + `PermissionHook` cover the same surface. |
| Per‑tenant capability embeddings | Breaks the global‑capability invariant. Tenant scope stays a post‑ANN filter (§2.2). |
| Sanitization inside capability providers | Output sanitization is `ArtifactBridge`'s job by design (post‑execution ownership). |
| New top‑level "security" crate | Would create a cross‑cutting registry. All hardening fits inside `capabilities/` and `mw/`. |

---

## 7. Execution order, gates, and verification

| # | Step | Gate before merge |
|---|---|---|
| 1 | §1.1 manifest fields + `compute_schema_hash` + unit tests | `cargo test -p agent-core capabilities::manifest` |
| 2 | §1.2 card provenance + §1.3 validator scanner | `cargo test -p agent-core capabilities::{card,validator}` |
| 3 | §1.4 MCP pinning in both providers | `cargo test -p agent-core capabilities::providers` |
| 4 | §1.5 router `with_tags` overload | `cargo test -p agent-core capabilities::semantic_router` |
| 5 | §2.1 `TenantScoped` wrapper + caller migration | `cargo build --workspace` (compile failure = caller missed) + `scripts/check-tenant-scoped-vector.mjs` |
| 6 | §3 env gate middleware + route wiring | `make verify-routes-doc` + manual `curl -i :8080/metrics` against `CONUSAI_ENV=production` → 404 |
| 7 | §4 evals suites | `just evals && just evals-security` |
| 8 | §5 arch.md update | `make verify` |

**Final gate:** full `cargo test --workspace`, `make verify`, `just evals`, `pnpm -w test` (front‑end smoke — unchanged but must still pass).

---

## 8. Effort estimate (recalibrated against actual code shape)

| Section | Original review | Adjusted (post‑codebase scan) | Reason for delta |
|---|---|---|---|
| §1 MCP & capability hardening | 3 h | **4 h** | extra provenance plumbing through `RegisteredToolStore`. |
| §2 Tenant‑scoped vectors | 1.5 h | **1 h** | content side already scoped; only wrapper + audit script. |
| §3 Public endpoint gating | 1 h | **1 h** | unchanged. |
| §4 Evals expansion | 4 h | **4 h** | unchanged. |
| §5 Docs | included | **0.5 h** | unchanged. |
| **Total** | 9.5 h | **10.5 h** | small upward adjustment for provenance store wiring. |

Token budget for execution (model‑side, generation only): **~45k**.

---

## 9. Risk register

| Risk | Mitigation |
|---|---|
| Adding optional fields to `ToolManifest` breaks deserialization of existing TOMLs | All new fields `#[serde(default)]`; `schema_version` already supports `1.0`/`2.0` — increment to `2.1` only inside `default_schema_version()`. Existing `1.0` manifests load unchanged. |
| `for_tenant` migration misses a caller | Compile‑time enforcement via `pub(crate)` + grep script in CI. |
| Env gate returns 401 and leaks endpoint existence | Return `404 NOT_FOUND` (deliberate). |
| Injection scanner false‑positives flood `last_error` | Warnings, not errors; logged as `warn:` prefix and surfaced in UI but never block. |
| Schema‑hash drift after legitimate version bump | Bump `manifest.version` + super‑admin re‑approves via `CapabilityAdmin::set_approved`. |

---

## 10. Frontend impact

**None.** `CapabilityRendererRegistry`, `createChatStream`, shared runes in `packages/ui` are unaffected. The `/super-admin/capabilities/*` Askama templates gain two new read‑only cells (provenance, schema_hash) — single template diff, no JS changes.
