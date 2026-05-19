# ConusAI Platform — Gap Remediation Plan

> Source: [arch.md §13 Identified Gaps](arch.md#13-identified-gaps) (2026-05-19 audit).
> Companion: [project-instructions.md](project-instructions.md).
>
> This plan is a concrete, file-level, step-by-step remediation for every drift item flagged in the architecture audit. Each phase is independently shippable and gated by a verification checklist.
>
> **Operating posture: aggressive.** Delete dead code on sight — do not gate removals on "someone might use it later." Default action for every stub, no-op, unused dep, dead route, dead feature flag, dead test fixture, and dead module is **delete in the same PR** that touches the surrounding code. Bugs are fixed, never "tracked." If a feature has no working implementation and no committed consumer, it is removed; if it is removed in error, `git revert` is the recovery path — not perpetual rot.

> **Architectural invariants (apply to every phase).**
> - `AgentBuilder` is the single composition root. New capabilities, embedding services, identity providers, and trace sources wire in via builder methods (`with_capability_provider`, `with_semantic_router`, `with_identity_provider`, …) — **never** scattered `new()` calls or globals.
> - Canonical capability surface — `Agent`, `AgentBuilder`, `CapabilityProvider`, `CapabilityFactory`, `CapabilityRegistry`, `SemanticCapabilityRouter`, `DynamicPromptCapability`, `ArtifactBridge` — is preserved verbatim. Rename = breaking change; do not rename in cleanup PRs.
> - Prefer Rig 0.36 idioms over hand-rolled equivalents: `CompletionModel::stream()`, `rig::embeddings::Embedder`, `rig::embeddings::fastembed::FastEmbed`. Hand-rolled is allowed only when Rig's abstraction hides a required knob (dim, prefix, batch).
> - Error boundaries: `thiserror` inside crate public APIs, `anyhow` at the binary edge. Do not mix.
> - Hot paths that take a registry lock prefer `dashmap` or `tokio::sync::RwLock` with fine-grained keys over coarse `Mutex`.

### Effort envelope

- **Total estimate:** 42–55 AI-hours across all phases (implementation + tests + migration runbook + CI hardening + one full verification pass).
- **LLM-assisted token budget:** ~450k–750k input tokens.
- **Highest-leverage phases:** **0** (unlocks everything by shrinking surface), **3** (eliminates a correctness + cost class of bugs), **5** (removes a silent correctness bug).
- **Recommended first PR sequence:** Phase 0 → Phase 3 → Phase 5 → remaining phases in parallel where independent (docs can move alongside code).

---

## Execution Order at a Glance

| # | Phase | Action | Risk | Touches |
| - | --- | --- | ---- | ------- |
| 0 | Dead-code sweep (`cargo machete`, `cargo udeps`, `#[allow(dead_code)]` purge, dead routes, dead Cargo features) | **DELETE** | Low | workspace-wide |
| 1 | Zitadel verification — lock in introspection, **delete** every JWKS/`openidconnect` reference | DELETE + FIX | Med | `identity/zitadel.rs`, `Cargo.toml`, `arch.md` |
| 2 | **Delete** dead workspace deps (`sqlx`, `openidconnect`) and add CI guard | DELETE | Low | root `Cargo.toml`, crate manifests |
| 3 | Fix embedding-dim mismatch bug (Qdrant 768 vs OpenAI 1536) — fail-loud on conflict | FIX | High | `qdrant_vector.rs`, `indexing/*`, migration script |
| 4 | Fix plan-clamp duplication bug — centralise in `mw/plan.rs`, **delete** ad-hoc clamps | DELETE + FIX | Med | `mw/plan.rs`, every protected handler |
| 5 | Fix silent-no-op hot-reload bug — implement `reload_one` or **delete** the listener | DELETE-OR-FIX | Med | `providers/capability_spec.rs`, `main.rs` |
| 6 | Fix `TraceReplayCapability` always-error bug — implement real source or **delete** the capability | DELETE-OR-FIX | Low | `capabilities/trace_replay.rs` |
| 7 | Document `rustfs-admin` as a first-class crate | DOC | Doc | `arch.md` §4 |
| 8 | Document `apps/backend/evals` harness | DOC | Doc | `arch.md` §1.2 + §4.1 |
| 9 | Enumerate Tauri shell modules in arch.md §7.2 | DOC | Doc | `arch.md` §7.2 |
| 10 | Auto-generate route tables — **delete** the hand-maintained route lists | DELETE + FIX | Doc | `project-instructions.md` §6, helper script |

Phases 0–6 are **code changes** (delete first, then fix). Phases 7–9 are **doc backfill**; Phase 10 deletes hand-maintained route documentation in favour of generated truth.

---

## Phase 0 — Dead-code sweep (precondition for everything else)

**Goal:** before any feature work, **delete** dead code so subsequent phases edit a smaller, honest surface.

### Steps (do all in one PR; revert in pieces if anything regresses)

1. Install + run:
   ```sh
   cargo install cargo-machete cargo-udeps
   cargo machete --with-metadata
   cargo +nightly udeps --workspace --all-targets
   ```
   **Delete** every dep flagged by either tool from the offending `Cargo.toml`. No exceptions for "might use later."
2. **Delete every `#[allow(dead_code)]`** in `apps/backend/crates/**`. If the compiler then warns, delete the item it points at — do not re-add the allow.
3. **Delete unused Cargo features.** Audit `[features]` in every crate manifest; remove any feature with no `cfg(feature = "...")` consumer (`rg 'feature = "X"' --type rust`).
4. **Delete unused public exports.** `cargo public-api --diff-git-checkouts HEAD~30 HEAD` — anything `pub` that has no in-tree call site and is not on a documented public surface (gateway routes, `@conusai/sdk` types) becomes `pub(crate)` or is deleted.
5. **Delete dead routes / handlers.** Cross-reference `routes/mod.rs` against `agent-gateway/tests/**` and `apps/web/src/lib/server/sdk.ts`. Any handler with zero call sites in tests **and** zero client references is deleted, not preserved.
6. **Delete commented-out code.** `rg '^\s*//\s*(let|fn|use|impl|struct|pub)\b' apps/backend/crates` — manually review and remove. Git history is the archive.
7. **Delete `.bak`, `.orig`, `_old.*`, `_deprecated.*` files** from the tree.
8. **Delete stale test fixtures.** Any file under `tests/fixtures/` or `e2e/fixtures/` not referenced by a live test is deleted.
9. **Delete dead env vars.** `rg 'env::var\("' apps/backend/crates` — cross-reference against `.env.example` and `docker-compose.yml`; delete vars read by no code or referenced by no deployment.
10. **Delete stub trait impls that return `unimplemented!()` / `todo!()` / `Err("not implemented")`** unless the consuming phase below promises an implementation in this PR cycle. If no consumer plans to implement, delete the trait, the impl, **and** the consumer chain in one commit.
11. **Test-scope refinement (do NOT over-delete).** Tests are first-class consumers. An item is dead **only** if it has no reference in any of: `**/*test*/**`, `**/*_test.rs`, `crates/agent-gateway/tests/**`, the `@conusai/sdk` surface, the `apps/web` SDK client, or a documented gateway route. Items used *only* by tests move under `#[cfg(test)]` (or a `pub(crate)` test-only module) instead of being deleted.
12. **Stale TODO/FIXME sweep.** `make verify-no-commented-code` greps for `// TODO`, `// FIXME`, `// XXX` and fails when `git blame` shows the line is >30 days old without a linked issue URL. Resolve or delete — do not extend the grace period.

### Verification

- `cargo build --workspace --all-targets` clean with `RUSTFLAGS="-D warnings -D dead_code"`.
- `cargo machete` exit 0.
- `cargo +nightly udeps --workspace` exit 0.
- `git diff --stat HEAD~1` shows net-negative LOC.
- CI re-runs in subsequent PRs treat new `dead_code` / `unused_imports` warnings as errors.

---

## Phase 1 — Lock in Zitadel introspection, delete JWKS path (Gap #1)

**Goal:** make arch.md and runtime agree by **picking one path and deleting the other**. The runtime uses `POST /oauth/v2/introspect`; arch.md historically described local JWKS verification via `openidconnect` 3.

**Decision (final, no debate):** keep introspection, delete every reference to `openidconnect`, JWKS, JWK caches, and JWT-local-verification paths. Introspection is simpler, supports opaque tokens, respects Zitadel revocation, and removes a heavy dep.

### Steps

1. **Confirm runtime in code** — `apps/backend/crates/agent-core/src/identity/zitadel.rs`:
   - `ZitadelProvider::verify_token` issues `POST {ZITADEL_DOMAIN}/oauth/v2/introspect` with `application/x-www-form-urlencoded` body `token=<bearer>` and `Authorization: Basic base64(client_id:client_secret)`.
   - Confirm response is parsed via `IntrospectionResponse { active, sub, exp, "urn:zitadel:iam:org:id", "urn:zitadel:iam:org:project:roles", "urn:conusai:plan_tier", "urn:conusai:subscription_status" }`.
2. **Delete the JWKS path entirely.** `rg -l 'openidconnect|JwkSet|DecodingKey|jwks' apps/backend/crates` — every match becomes `git rm` (whole file) or a delete-block edit. Remove:
   - any `JwksCache`, `JwkSetClient`, or `decode_jwt_with_jwks` helper;
   - the `openidconnect` import block in `zitadel.rs`;
   - any `fn verify_jwt_local` and its tests.
3. **Cache introspection results** (new): wrap the call in a `moka::future::Cache<TokenHash, ResolvedIdentity>` with TTL = `min(exp - now, 60s)`.
   - Key = blake3 of the raw token bytes — never the token itself.
   - Invalidate on 401 from any downstream call.
4. **Surface mgmt-API config** — keep `ZITADEL_MGMT_PAT` only for org/user provisioning (`routes/admin_tenants.rs`); never for request-path verification. Delete any other consumer.
5. **Extract an `IdentityProvider` trait** in `agent-core/src/identity/mod.rs`:
   ```rust
   #[async_trait]
   pub trait IdentityProvider: Send + Sync {
       async fn verify_token(&self, token: &str) -> Result<ResolvedIdentity, IdentityError>;
       async fn invalidate(&self, token: &str);
   }
   ```
   `ZitadelProvider` implements it; the gateway holds `Arc<dyn IdentityProvider>` and `AgentBuilder::with_identity_provider(...)` is the only wiring path. This lets tests inject a `MockIdentityProvider` without `wiremock`, and makes future provider swaps (Auth0, Keycloak, local-dev) one-file changes.
6. **Update arch.md §4.4, §6, §12.5** — replace every "JWKS / `openidconnect`" mention with "introspection over reqwest, cached ≤60 s". Add a table of the four custom claims and where each is consumed.
7. **Update [project-instructions.md §4](project-instructions.md)** — already correct; verify.

### Verification

- `cargo build -p agent-core -p agent-gateway` clean.
- Manual: with `CONUSAI_AUTH_PROVIDER=zitadel`, `curl -H "Authorization: Bearer <token>" $GATEWAY/v1/capabilities` returns 200 once, and subsequent identical calls within 60 s show zero new introspection requests in Zitadel logs.
- Add an integration test under `crates/agent-gateway/tests/identity_zitadel.rs` using `wiremock` to stub `/oauth/v2/introspect` and assert the cache hit/miss counters.

---

## Phase 2 — Delete dead workspace deps (Gap #3)

**Goal:** `sqlx` 0.8 and `openidconnect` 3 are declared but linked into nothing after Phase 1. Delete them.

### Steps

1. **Confirm zero runtime usages:**
   ```sh
   rg "use sqlx|sqlx::" apps/backend/crates --type rust
   rg "use openidconnect|openidconnect::" apps/backend/crates --type rust
   ```
   Expected: empty after Phase 1 + the Postgres removal. If anything matches, **delete the matching code first** (it is by definition vestigial), then rerun.
2. **Delete from root `Cargo.toml`** — remove `sqlx` and `openidconnect` from `[workspace.dependencies]`. Remove every `sqlx = { workspace = true }` / `openidconnect = { workspace = true }` from per-crate manifests.
3. **Run** `cargo update --workspace && cargo build --workspace --all-targets`. Commit the shrunken `Cargo.lock`.
4. **`evals` exception** — if `apps/backend/evals` genuinely needs Postgres for benchmark fixtures, declare `sqlx` only inside `apps/backend/evals/Cargo.toml [dev-dependencies]`. **Forbid** runtime crates from picking it up transitively.
5. **Permanent CI guard:** add a blocking `make verify-no-dead-deps` step that runs `cargo machete` + `cargo +nightly udeps --workspace --all-targets`. Fail the build on any reported unused dep — no allowlist.

### Verification

- `cargo machete` (or `cargo udeps`) returns no warnings about `sqlx` / `openidconnect`.
- `cargo tree -p agent-core | rg "sqlx|openidconnect"` returns nothing.
- Update [arch.md §12.10](arch.md#1210-workspace-dependency-corrections) — remove the "declared, runtime-unused" rows.

---

## Phase 3 — Standardise on local multilingual embeddings, fix dim mismatch (Gap #8)

**Goal:** today both Qdrant collections are created with `size=768` while the runtime can also be pointed at OpenAI `text-embedding-3-small` (1536-d) — the code silently truncates or fails depending on path. **Decision (final):** drop OpenAI from the embedding hot path entirely and standardise on Qdrant's recommended local multilingual model, `intfloat/multilingual-e5-large` (**1024-d**, 100+ languages, MIT, served via `fastembed-rs`). This removes a network dep, a paid API, and a whole class of multi-tenant data-egress concerns; quality on multilingual workloads beats `text-embedding-3-small` on MIRACL/MTEB. English-only deployments can opt into the lighter `BGESmallENV15` (384-d) but it is not the default.

**Rig integration (mandatory).** Use Rig 0.36's first-class fastembed support — depend on `rig = { version = "0.36", features = ["fastembed"] }` and wrap `rig::embeddings::fastembed::FastEmbed` in a thin `RigEmbeddingService` adapter that implements our `EmbeddingService` trait. This keeps `SemanticCapabilityRouter`, future RAG paths, and any Rig-native agent composition sharing one embedder. Fall back to raw `fastembed-rs` **only** if Rig's `Embedder` abstraction hides the required dim or `"query: " / "passage: "` prefix control — document the reason in the adapter if so.

### Steps

1. **Replace `EMBEDDING_BACKEND` with a single fastembed-backed service.** In `agent-core/src/indexing/embedding_service.rs`:
   ```rust
   pub enum EmbeddingModel {
       /// intfloat/multilingual-e5-large — 1024-d, 100+ languages (DEFAULT).
       MultilingualE5Large,
       /// BAAI/bge-small-en-v1.5 — 384-d, English-only opt-in.
       BgeSmallEnV15,
   }
   impl EmbeddingModel {
       pub fn dims(self) -> u64 { match self { Self::MultilingualE5Large => 1024, Self::BgeSmallEnV15 => 384 } }
       pub fn fastembed(self) -> fastembed::EmbeddingModel { … }
   }
   pub trait EmbeddingService: Send + Sync {
       fn model(&self) -> EmbeddingModel;
       fn dims(&self) -> u64 { self.model().dims() }
       async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
   }
   ```
2. **Honour the E5 prompt convention.** `multilingual-e5-large` is asymmetric: prefix passages with `"passage: "` and queries with `"query: "` before calling `embed`. Centralise this in `FastEmbedService::{embed_documents, embed_query}`; **delete** any call site that goes through a single `embed()` with no role.
3. **Delete the OpenAI embedding path.** `rg -l 'openai.*embedding|EMBEDDING_BACKEND|OpenAIEmbeddings' apps/backend/crates` → `git rm` the module(s), drop the `openai` embedding feature from `agent-core/Cargo.toml`, remove `OPENAI_API_KEY` consumers in the embedding chain (chat/agent paths keep it). Drop `EMBEDDING_BACKEND` and `OPENAI_EMBEDDING_DIMS` from `.env.example` and `docker-compose.yml`.
4. **New env knobs (single source of truth):**
   - `EMBEDDING_MODEL=multilingual-e5-large` (default) | `bge-small-en-v1.5`.
   - `EMBEDDING_CACHE_DIR=/var/lib/conusai/fastembed` — ONNX weights download location; pre-warmed in the Docker image so first-boot does not stall.
   - `EMBEDDING_MAX_BATCH=32` — caps tokens-per-batch to keep p99 latency bounded on CPU.
5. **Qdrant collection bootstrap** (`store/qdrant_vector.rs::ensure_collection`):
   - Read dims from the active `EmbeddingService` at construction time.
   - Suffix collection name with dim when it diverges from the legacy 768-d default: `capability_embeddings_d1024`, `content_embeddings_d1024` (and `_d384` for BGE). Keeps existing 768-d deployments addressable for migration.
   - On startup, if a collection with the wrong dim exists, **fail loudly** with a remediation hint (`re-embed via scripts/reindex.sh --model multilingual-e5-large`); never silently re-create.
   - Use `Distance::Cosine` (E5 is trained for cosine; do **not** use dot-product).
6. **Add a re-embed script** `scripts/reindex.sh` that:
   - Snapshots the source collection.
   - Re-streams every `WorkspaceNode` and `CapabilityCard` through the new `EmbeddingService` (with the correct `passage: ` prefix).
   - Writes to the new dim-suffixed collection.
   - Atomically aliases the new collection (Qdrant `update_aliases`) so search continues uninterrupted.
7. **Pre-warm in Docker.** `apps/backend/Dockerfile` runs `cargo run -p agent-gateway --bin warm-embeddings` at build time to download and cache the ONNX weights into the image layer — no cold-start network call in production.
8. **Wire the dim into payload-index creation** (no change — payload indexes are dimension-agnostic).

### Verification

- Unit test in `qdrant_vector.rs` asserting `ensure_collection` refuses to attach to a 768-d legacy collection when service reports 1024.
- Boot the gateway against an empty Qdrant with `EMBEDDING_MODEL=multilingual-e5-large` (default) — observe `capability_embeddings_d1024` created with `Cosine` distance.
- Multilingual smoke test: embed `["passage: 人工知能", "passage: artificial intelligence", "passage: dirbtinis intelektas"]`; assert pairwise cosine sim ≥ 0.85.
- `rg 'openai.*embedding|EMBEDDING_BACKEND' apps/backend/crates` returns nothing.
- Add the model + dim to `/metrics` (`embedding_dims{model="multilingual-e5-large"}=1024`).

---

## Phase 4 — Centralise plan clamp in `mw/plan.rs` (Gap #7)

**Goal:** today `mw/plan.rs` only validates the plan tier exists; each handler re-applies `max_tokens` and `max_turns` independently. Move clamping into the middleware so handlers can trust `TenantContext.plan`.

### Steps

1. **Promote `PlanLimits`** in `agent-core/src/context/tenant.rs`:
   ```rust
   pub struct PlanLimits {
       pub max_tokens: u32,
       pub max_turns: u32,
       pub default_alias: &'static str,
       pub max_tools_per_turn: u32,
       pub max_invokes_per_turn: u32,
       pub daily_quota: u64,
   }
   impl PlanTier { pub fn limits(self) -> PlanLimits { … } }
   ```
2. **In `mw/plan.rs`**:
   - After resolving `PlanTier` from `IdentityContext`, attach `PlanLimits` to the `Extensions` so handlers extract it via `Extension<PlanLimits>`.
   - Clamp request-supplied `max_tokens` / `max_turns` before the handler ever sees them (rewrite the JSON body in a `from_request_parts` extractor or in `body_clamp_layer`).
3. **Remove ad-hoc clamping** from:
   - `routes/chat.rs` (`min(req.max_tokens.unwrap_or(plan.max_tokens), plan.max_tokens)`)
   - `routes/agent.rs`
   - `chains/llm_chain.rs` invocation path
4. **`RouterQuotaLayer`** (in `mw/router_quota.rs`) reads from `PlanLimits` instead of env vars; env vars become **fallback defaults** only when no `IdentityContext` is attached (internal routes).
5. Add unit tests under `crates/agent-gateway/tests/plan_clamp.rs` that send `max_tokens=999999` on a Free-tier token and assert the request reaching the handler has `max_tokens=plan.free.max_tokens`.

### Verification

- All chat/agent handlers compile after removing the local clamp.
- `curl -d '{"max_tokens":99999, "messages":[…]}'` on a Free plan returns 200 with `usage.completion_tokens <= plan_free.max_tokens`.
- Prometheus counter `plan_clamp_total{tier,parameter}` increments on every clamp.

---

## Phase 5 — Fix silent-no-op hot-reload bug (Gap #5)

**Goal:** the boot listener in `agent-gateway/src/main.rs` subscribes to `RedbMetadataStore::subscribe_spec_changes()` and calls `CapabilitySpecFactory::reload_one(namespace, tool)`, but `reload_one` is a no-op stub since the Postgres removal. This is a **silent correctness bug**: admins update specs, the system reports success, nothing reloads.

**Decision (final):** implement against redb. **If implementation is deferred for any reason, delete the listener, the broadcast channel, and `subscribe_spec_changes` in the same PR** — a wired no-op is a worse bug than a missing feature.

### Steps

1. **Introduce `CapabilityMutationService`** (new, in `agent-core/src/capabilities/mutation.rs`) that owns both the redb mutation **and** the `notify_spec_change` emission. Every admin mutation route calls this service — never the metadata store directly — so it is structurally impossible to add a future mutation path that forgets to notify.
2. **In `agent-core/src/capabilities/providers/capability_spec.rs`**:
   - Replace the stub `reload_one` with:
     ```rust
     pub async fn reload_one(&self, namespace: &str, tool: &str) -> anyhow::Result<()> {
         let spec = self.metadata.get_capability_spec(namespace, tool).await?
             .ok_or_else(|| anyhow!("spec not found: {namespace}/{tool}"))?;
         let provider = self.materialise(&spec).await?;
         self.registry.lock().await.replace_provider(namespace, tool, provider);
         self.metrics.spec_reload_total.with_label_values(&[namespace, "success"]).inc();
         Ok(())
     }
     ```
   - Add `materialise(&CapabilitySpec)` covering all five strategies: `dynamic_prompt`, `prompt` (chain), `wasm`, `native`, `remote_mcp`.
   - `CapabilityRegistry::replace_provider` should use `dashmap` or `tokio::sync::RwLock` keyed on `(namespace, tool)` — a coarse `Mutex` over the whole registry will serialise hot-path lookups under reload contention.
3. **Add an admin endpoint** `POST /admin/capabilities/{namespace}/{tool}/reload` that calls `reload_one` directly — useful for testing, for ops without round-tripping through redb mutation, and as a safe escape hatch if the redb broadcast ever shows latency or ordering issues under contention.
4. **All admin mutation routes** (`admin_capabilities.rs::{create, update, delete, toggle_enabled}`) go through `CapabilityMutationService` (step 1); the service guarantees `notify_spec_change(namespace, tool)` fires.
5. **Add a smoke test** under `crates/agent-gateway/tests/spec_reload.rs`:
   - Create a chain capability via `POST /admin/capabilities`.
   - Assert `GET /v1/capabilities` lists it within 200 ms (covers redb broadcast latency).
   - Update its prompt; assert next invocation reflects the new prompt.

### Verification

- `spec_reload_total{result="success"}` increments on every admin mutation.
- Removing a capability via `DELETE /admin/capabilities/...` removes it from `/v1/capabilities` without a process restart.

**If hot-reload is deferred:** `git rm` the `subscribe_spec_changes` subscription block in `agent-gateway/src/main.rs`, `git rm` the `notify_spec_change` callers, **and** `git rm` the broadcast channel in `RedbMetadataStore`. Document the removal (not the limitation) in [arch.md §12.6](arch.md). Never leave a listener wired to a no-op.

---

## Phase 6 — Fix `TraceReplayCapability` always-error bug (Gap #6)

**Goal:** `WorkspaceNodeTraceSource` in `capabilities/trace_replay.rs` always returns `Err("not implemented")` yet the capability is still registered by `with_all_factories` and advertised via `/v1/capabilities`. This lies to callers — every invocation 500s. **Implement or delete; no third option.**

### Steps

1. **Implement `AuditTraceSource`** in `agent-core/src/capabilities/trace_replay.rs`:
   - Accept `Arc<RedbMetadataStore>` + a `TraceLocator { tenant, thread_id, message_seq_range }`.
   - Read trace events from the existing `audit_events` table (keyed by `(tenant, ts_micros, event_id)`).
   - Stream the matching `AuditEvent`s back as a JSON array.
   - **Delete** the stub `WorkspaceNodeTraceSource` in the same commit. Do not leave both.
2. **Wire the new source** in `agent-gateway/src/state.rs`: `TraceReplayFactory::new(audit_source)`.
3. **No Cargo feature flag.** Trace replay is either present and working, or absent. If a deployment forbids replay, gate via runtime config (`CAPABILITY_TRACE_REPLAY_ENABLED=false`) which **also unregisters the capability** from `/v1/capabilities`. Do not advertise capabilities that 500.
4. **If implementation is deferred:** `git rm capabilities/trace_replay.rs`, remove `TraceReplayFactory` from `with_all_factories`, delete the route registration, delete the SDK type, and delete the UI affordance. Same PR.

### Verification

- `POST /v1/agent/completions` with `{"capability":"trace_replay","args":{"thread_id":"…","range":[0,10]}}` returns the actual audit events for that thread in order.
- Negative test: cross-tenant call returns `403`.

---

## Phase 7 — Document `rustfs-admin` as first-class crate (Gap #2)

**Goal:** the `apps/backend/crates/rustfs-admin` crate (bootstrap, IAM, presign, quotas, bucket notifications) is invisible in `docs/`.

### Steps

1. **Add `arch.md §4.x — `rustfs-admin`** with:
   - File tree (`src/{bootstrap,iam,presign,quotas,notifications,error}.rs`).
   - Public types: `RustFsAdminClient`, `BootstrapPlan`, `IamPolicy`, `BucketNotificationConfig`.
   - Env vars it consumes (already enumerated in [project-instructions.md §9](project-instructions.md) under "RustFS bootstrap").
   - Sequence diagram: `RustFsAdminClient::bootstrap_storage` → create root bucket → create per-tenant prefix policy → install bucket notification webhook → seed `tenant_seeded` redb marker.
2. **Add `arch.md §4.x.1 — Tenant onboarding flow** showing the chain: `TenantOnboardingService` → `RustFsAdminClient::ensure_tenant_iam` → `CredentialStore::put_credentials` (AES-256-GCM).
3. **Update [project-instructions.md §2.1](project-instructions.md)** — the bullet already lists `crates/rustfs-admin`; expand its trailing comment to `bootstrap · per-tenant IAM · presign · quotas · bucket notifications`.

### Verification

- Doc-lint: `markdownlint docs/arch.md` clean.
- Cross-link: `arch.md §12.3` references the new `§4.x`.

---

## Phase 8 — Document `apps/backend/evals` harness (Gap #9)

**Goal:** `apps/backend/evals` is a Cargo workspace member but absent from arch.md §1.2 and §4.1.

### Steps

1. **Walk the tree:**
   ```sh
   find apps/backend/evals -maxdepth 3 -type d
   ```
2. **Add `arch.md §4.x — Eval Harness** documenting `evals/runners/*` (agent harness, chain harness) and `evals/scorers/*` (exact match, BLEU, custom rubric scorers) plus the CLI entry (`cargo run -p evals -- run --suite <name>`).
3. **Update [project-instructions.md §2.1](project-instructions.md)** — the bullet `evals  ← runners + scorers` is already present; verify it survives the next regen.

### Verification

- `cargo run -p evals -- --help` exits 0 and prints the documented subcommands.

---

## Phase 9 — Enumerate Tauri shell modules (Gap #10)

**Goal:** arch.md §7.2 lists the Tauri plugins but not the per-module breakdown of `apps/browser-shell/src-tauri/src/*.rs`.

### Steps

1. Add to `arch.md §7.2`:
   - `chat_stream.rs` — SSE proxy (`text/event-stream` → Tauri `chat:chunk:<id>` events) so WKWebView buffering does not stall tokens.
   - `device_auth.rs` — Stronghold-backed `X-Device-Token` rotation.
   - `oidc_auth.rs` — PKCE flow against Zitadel; opens the system browser via `open` 5.
   - `recorder.rs` — DOM event capture forwarded over a single injected JS bridge.
   - `registration.rs` — initial device registration handshake against `/admin/devices/register`.
   - `tabs.rs` — multi-tab state for the desktop shell.
   - `telemetry.rs` — opt-in OTLP forwarding to `OTLP_ENDPOINT`.
   - (debug + macOS only, `e2e` feature) `tauri-plugin-webdriver-automation` 0.1.3 — W3C WebDriver server for WKWebView.
2. Mirror the list as a sentence in [project-instructions.md §11](project-instructions.md) — already present (`chat_stream, device_auth, oidc_auth, recorder, registration, tabs, telemetry`); keep them aligned.

### Verification

- The bullet list in arch.md is a strict subset of `ls apps/browser-shell/src-tauri/src/*.rs`.

---

## Phase 10 — Regenerate route tables from `routes/mod.rs` (Gap #4)

**Goal:** route documentation drifts every time `routes/mod.rs` changes. Automate it.

### Steps

1. **Add `scripts/dump-routes.sh`:**
   ```sh
   #!/usr/bin/env bash
   set -euo pipefail
   cargo run -p agent-gateway --bin agent-gateway -- --dump-routes > docs/_routes.generated.md
   ```
2. **Implement `--dump-routes`** in `agent-gateway/src/main.rs`:
   - Build all four routers (`public`, `protected(quota)`, `admin`, `internal`) via the existing builders.
   - Walk `axum::Router::method_routes` (or maintain a static `pub const ROUTES: &[Route]` populated by `#[utoipa::path]` derive).
   - Print as Markdown grouped by router with method + path + auth requirement.
3. **CI check** — `make verify-routes-doc` runs the dump and `diff -u docs/_routes.generated.md docs/_routes.expected.md`. Fail on drift.
4. **Re-render the §6 section of `project-instructions.md`** by including `docs/_routes.generated.md` (or paste-replace on change). The presence/auth/quota columns must match middleware order from [arch.md §12.8](arch.md#128-middleware-stack-outermost--innermost-on-the-protected-router).

### Verification

- After adding any new route, CI fails until `docs/_routes.expected.md` is updated.
- `curl $GATEWAY/openapi.json | jq '.paths | keys | length'` matches the row count in `_routes.generated.md`.

---

## Cross-cutting Tasks

### CI additions (all blocking, no allowlists)

- `cargo machete` — fail on any unused dep (phase 0 + 2).
- `cargo +nightly udeps --workspace --all-targets` — fail on any unused dep transitively (phase 0 + 2).
- `RUSTFLAGS="-D warnings -D dead_code -D unused_imports -D unused_variables"` on the main `cargo build` (phase 0).
- `cargo clippy --workspace --all-targets -- -D warnings -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::print_stdout` — forbids stubs and debug residue from re-entering the tree.
- `make verify-routes-doc` — phase 10.
- `make verify-no-commented-code` — grep for `// TODO`, `// FIXME`, `// XXX` older than 30 days via `git blame`; require a tracked issue link or delete.
- New integration tests:
  - `crates/agent-gateway/tests/identity_zitadel.rs` — phase 1.
  - `crates/agent-gateway/tests/plan_clamp.rs` — phase 4.
  - `crates/agent-gateway/tests/spec_reload.rs` — phase 5.
  - `crates/agent-gateway/tests/trace_replay.rs` — phase 6.

### Observability deltas

- New Prometheus counters: `zitadel_introspection_cache_{hits,misses}`, `plan_clamp_total{tier,parameter}`, `spec_reload_total{namespace,result}`, `embedding_dims{backend}`.
- New tracing spans: `identity.introspect`, `capability.spec.reload`, `embedding.embed{backend,dims}`.

### Migration runbook

1. Phase 3 will require a Qdrant re-embed for any tenant switching backends — schedule maintenance windows. Provide `scripts/reindex.sh` with `--dry-run`.
2. Phase 2 ships a smaller `Cargo.lock` — communicate to anyone with vendored builds.

### Out of Scope (deliberate)

- ADR 0003 supersession (already historical).
- Replacing redb with a different KV — orthogonal.
- Adding new LLM providers — orthogonal.
- Frontend redesign — covered by separate `ui-plan.md`.

---

## Definition of Done

- Every gap in [arch.md §13](arch.md#13-identified-gaps) is **closed** (code + doc + test) **or deleted** (capability/route/dep removed entirely). "Deferred" is **not** an acceptable outcome — if a gap cannot be closed, its surface area is removed.
- `docs/arch.md` §13 ends this cycle empty (or is itself deleted as a section).
- `project-instructions.md` is regenerated from the route dump and is ≤12000 chars.
- Net LOC delta for the workspace is **negative**.
- All new or significantly changed public APIs in `agent-core` ship with **doc-tests or unit tests showing composition via `AgentBuilder`** — no `new()`-only examples.
- `CapabilityProvider` + `CapabilityFactory` contract (lifecycle, error semantics, idempotency expectations, registration via builder) is documented in `docs/capability-authoring-guide.md` so dynamic-prompt / WASM / remote-MCP authors have a stable extension point.
- CI blocks on: `cargo machete`, `cargo udeps`, `-D warnings -D dead_code`, `clippy::{todo,unimplemented,dbg_macro,print_stdout} = deny`, `make verify-routes-doc`, `make verify-no-commented-code`, and the four new integration suites — every PR, no overrides.
