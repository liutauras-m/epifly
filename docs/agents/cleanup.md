# Aggressive Cleanup Plan — Deprecated Code & Packages

**Mode:** No backward compatibility. Delete first, refactor callers second. Each phase ends with `cargo build --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` clean.

**Scope:** `apps/backend` (Rust workspace) and `apps/web` (Next.js).

---

## Phase 0 — Baseline (sanity)

- [ ] `cargo update --workspace` to refresh lockfile within current SemVer ranges
- [ ] Tag git HEAD as `pre-cleanup` for rollback safety
- [ ] Run full test suite, capture passing baseline

---

## Phase 1 — Delete the deprecated `/v1/threads` REST API

**Target:** All 5 endpoints + the `deprecated()` helper. Direct-thread management is replaced by workspace-node-scoped conversation flows (already present per [conversation.rs:3](apps/backend/crates/agent-core/src/context/conversation.rs#L3)).

### Delete
- [ ] `rm apps/backend/crates/agent-gateway/src/routes/threads.rs` (entire file, ~340 LOC)
- [ ] Remove `mod threads;` from [routes/mod.rs](apps/backend/crates/agent-gateway/src/routes/mod.rs)
- [ ] Remove all 5 `.route("/v1/threads...")` lines from `routes/mod.rs` (lines ~136–144)
- [ ] Remove `threads::*` from the `utoipa::OpenApi` `paths(...)` macro
- [ ] Remove `Thread` / `Message` request/response schemas from the OpenAPI `components(schemas(...))` if not reused

### Refactor callers
- [ ] Frontend: grep `apps/web` for `/v1/threads` and delete any client code (zero matches expected — confirm)
- [ ] Delete any integration tests under `apps/backend/crates/agent-gateway/tests/` that hit `/v1/threads`
- [ ] Delete deprecated thread fixtures from `apps/backend/evals/datasets/`

### Storage
- [ ] If the `ThreadStore` / `MessageStore` traits are *only* used by these routes, delete them too:
  - Audit usages in `agent-core` — keep only what `conversation.rs` (workspace-node flow) needs
  - Remove unused trait methods, in-memory impls, and SQL impls
- [ ] Drop any thread-only DB migrations / table DDL not referenced by node-scoped flows

### Verify
- [ ] `cargo build && cargo test -p agent-gateway`
- [ ] OpenAPI snapshot test updated; `Deprecation: true` header logic gone

---

## Phase 2 — Replace deprecated `serde_yaml` (0.9.34+deprecated)

**Target:** Single usage in [tools/manifest.rs:36](apps/backend/crates/agent-core/src/tools/manifest.rs#L36) plus 7 capability YAML files.

### Option A (recommended — aggressive): drop YAML, switch capability manifests to TOML
Rationale: Rust ecosystem has no maintained YAML crate that is a true drop-in. TOML is already a workspace dependency (via Cargo) and `figment` is already configured for it.

- [x] Convert `apps/backend/capabilities/*/capability.yaml` → `capability.toml` (done)
- [ ] In [manifest.rs](apps/backend/crates/agent-core/src/tools/manifest.rs):
  - Rename `from_yaml` → `from_toml`; use `toml::from_str`
  - Rename `from_yaml_file` → `from_toml_file`
- [ ] Update capability loader to scan `capability.toml`
- [ ] `apps/backend/Cargo.toml`: remove `serde_yaml = "0.9"` workspace dep
- [ ] `crates/common/Cargo.toml` and `crates/agent-core/Cargo.toml`: remove `serde_yaml.workspace = true`
- [ ] `figment` features in workspace `Cargo.toml`: drop `"yaml"`, keep `["env", "toml"]`
- [ ] `git rm` the seven `.yaml` files

### Option B (fallback): swap to `serde_yml` (community-maintained fork)
Only use if TOML migration is rejected. Drop-in: `s/serde_yaml/serde_yml/g`.

### Verify
- [ ] All capability tests pass
- [ ] `grep -r serde_yaml apps/backend` returns 0 matches
- [ ] `cargo tree -p serde_yaml` (workspace root) returns "package not found"

---

## Phase 3 — Eliminate dual-version dependencies

**Target:** Resolve `Cargo.lock` showing two majors of `axum`, `reqwest`, and `thiserror`.

### `axum` 0.7 + 0.8 → 0.8 only
- [ ] `cargo tree -i axum:0.7.9 --workspace` to find what pulls 0.7
- [ ] Likely culprits: `utoipa-swagger-ui` v9 (check), or older `axum-extra`
- [ ] Bump `axum-extra` / `tower-cookies` / any axum-adjacent crate to versions tracking 0.8
- [ ] If `utoipa-swagger-ui = 9` still pulls axum 0.7, upgrade to a version that supports 0.8 or vendor the trivial swagger handler

### `reqwest` 0.12 + 0.13 → 0.13 only
- [ ] `cargo tree -i reqwest:0.12.28 --workspace`
- [ ] `rig-core 0.36` likely pulls 0.13; bump our workspace dep to `reqwest = "0.13"` and update call sites (mostly identical API)

### `thiserror` 1 + 2 → 2 only
- [ ] `cargo tree -i thiserror:1.0.69 --workspace` — find transitive holdouts
- [ ] If a dep is unmaintained, file an upgrade or fork; otherwise wait it out (low blast radius)

### Verify
- [ ] `cargo tree --workspace --duplicates` produces minimal output
- [ ] `cargo build --workspace` clean

---

## Phase 4 — Major-version upgrades (breaking, no compat shims)

Each is its own atomic commit. Touch only what compile errors force.

### 4a — `wasmtime` 29 → 36 (7 majors)
- [ ] Bump `wasmtime` and `wasmtime-wasi` in workspace `Cargo.toml` to `"36"`
- [ ] Read [wasmtime CHANGELOG](https://github.com/bytecodealliance/wasmtime/blob/main/RELEASES.md) for 30→36 breaking changes
- [ ] Files to touch: `apps/backend/capabilities/template-wasm/**` and any `wasmtime::*` import in `agent-core`
- [ ] Run capability sandbox tests

### 4b — `axum` 0.7 → 0.8 cleanup
- [ ] Already partially done in Phase 3. Verify all routes use 0.8-style typed extractors and `{path}` (curly) syntax
- [ ] `IntoResponse` impls — check signature changes

### 4c — `schemars` 0.8 → 1.x
- [ ] Bump dep
- [ ] `JsonSchema` derive: API mostly stable; check for `gen` → `generator` rename
- [ ] Re-snapshot any generated OpenAPI / JSON Schema fixtures

### 4d — `jsonwebtoken` 9 → 10
- [ ] Bump dep
- [ ] Check `Validation` / `DecodingKey` / `Algorithm` API drift
- [ ] Re-run auth integration tests

### 4e — `tower` 0.4 → 0.5 cleanup (if still dual-versioned after Phase 3)
- [ ] Mostly handled by upgrading `axum` and `tower-http`

### 4f — `askama` 0.12 → 0.16 (4 majors)
- [ ] Bump dep
- [ ] Templates affected: [ui/view.rs](apps/backend/crates/agent-gateway/src/ui/view.rs), [ui/handlers/app.rs](apps/backend/crates/agent-gateway/src/ui/handlers/app.rs), [ui/handlers/auth.rs](apps/backend/crates/agent-gateway/src/ui/handlers/auth.rs)
- [ ] 0.13+ moved to compile-time-only; check for runtime template features being used
- [ ] Update `Cargo.toml` features and any `askama_axum` integration

### 4g — `object_store` 0.11 → 0.13
- [ ] Bump; check S3 `ObjectStore` builder API
- [ ] Touches `capabilities/file-storage/`

### Verify each sub-phase
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean

---

## Phase 5 — Dead code & stale `#[allow(dead_code)]`

**Target:** 6 `#[allow(dead_code)]` annotations — many are bandaids for either Phase A LLM stubs or removable code.

- [ ] [routes/threads.rs:51](apps/backend/crates/agent-gateway/src/routes/threads.rs#L51) — gone after Phase 1
- [ ] [mw/request_id.rs:21](apps/backend/crates/agent-gateway/src/mw/request_id.rs#L21) — audit; if helper unused, delete
- [ ] [state.rs:22 / 27](apps/backend/crates/agent-gateway/src/state.rs#L22) — keep only `pub llm` (used after Phase 6); delete the other if truly unused
- [ ] [ui/view.rs:19](apps/backend/crates/agent-gateway/src/ui/view.rs#L19) and [ui/session.rs:42](apps/backend/crates/agent-gateway/src/ui/session.rs#L42) — audit & delete fields never read
- [ ] Run `cargo +nightly udeps --workspace` (or `cargo machete`) to find unused workspace deps; remove

---

## Phase 6 — Phase-A LLM stubs → real implementations (or delete)

These are not deprecated, but they are dead branches that violate the cleanup mandate. Either implement or excise.

- [ ] [anthropic.rs:118 stream()](apps/backend/crates/agent-core/src/llm/providers/anthropic.rs#L118) — replace single-chunk hack with native rig streaming
- [ ] [anthropic.rs:144 tool_loop()](apps/backend/crates/agent-core/src/llm/providers/anthropic.rs#L144) — implement using `ToolDispatcher`
- [ ] [provider.rs:50 default tool_loop](apps/backend/crates/agent-core/src/llm/provider.rs#L50) — remove default impl that returns `Unsupported`; force every provider to implement
- [ ] [config/mod.rs:40 OllamaProviderConfig](apps/backend/crates/common/src/config/mod.rs#L40) — DELETE entirely. Add back when Phase D actually starts.
- [ ] [config/mod.rs:42 OpenAiProviderConfig](apps/backend/crates/common/src/config/mod.rs#L42) — DELETE entirely. Add back when Phase E actually starts.
- [ ] Remove `LlmError::Unsupported` variant once no provider returns it

---

## Phase 7 — Frontend hygiene

- [ ] `cd apps/web && pnpm outdated` — capture report
- [ ] `next-auth ^5.0.0-beta.29` → upgrade to stable v5 (currently GA)
- [ ] `ai ^4.3.16` → audit for v5 (Vercel AI SDK)
- [ ] `tailwindcss ^3` → upgrade to v4 (breaking: new engine, CSS-first config)
- [ ] `@biomejs/biome ^2.0.0` → check latest; re-run `biome check --write .` after upgrade
- [ ] Delete any commented-out / dead components, unused imports surfaced by `biome check`
- [ ] `pnpm dedupe`

---

## Phase 8 — Documentation purge

- [ ] Delete `CLAUDE_1.md`, `CLAUDE_2.md`, `CLAUDE_3.md` if superseded by `CLAUDE-4.md` and `CLAUDE-MONOREPO.md`
- [ ] Delete `docs/agents/agents-plan.md` sections referencing removed `/v1/threads`
- [ ] Update `docs/arch.md` and `docs/frontend/api.md` to drop the deprecated endpoints

---

## Final verification (gate)

- [ ] `cargo build --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo tree --workspace --duplicates` — minimal
- [ ] `grep -rn "deprecated\|Deprecation" apps/backend/crates apps/web/src` — only legitimate (none expected)
- [ ] `grep -rn "Phase [B-Z]" apps/backend/crates` — should be 0 (stubs removed)
- [ ] `grep -rn "#\[allow(dead_code)\]" apps/backend/crates` — only justified annotations remain
- [ ] `pnpm -w build` (frontend)
- [ ] `pnpm -w lint`

---

## Order of operations (TL;DR)

1. **Phase 1** (delete `/v1/threads`) — biggest win, smallest risk
2. **Phase 2** (kill `serde_yaml`) — removes the only formally deprecated package
3. **Phase 5 + 6** (dead code, Phase A stubs) — easy LOC reduction
4. **Phase 3** (dedupe versions) — sets stage for major upgrades
5. **Phase 4a–4g** (major upgrades) — one PR each, smallest blast radius first (4d → 4c → 4b → 4f → 4g → 4a)
6. **Phase 7** (frontend) — independent track, can run in parallel
7. **Phase 8** (docs) — final sweep

**Estimated net deletion:** ~500 LOC backend + 7 YAML files + 3 dependency entries.
