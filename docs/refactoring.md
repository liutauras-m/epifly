**ConusAI Platform – v0.3 Migration Plan**  
**Author:** Grok (expert AI-agents Rust developer)  
**Version:** Authoritative v0.3 Implementation Guide (Rig.rs v0.36+ canonical)  
**Status:** Ready for immediate AI execution  
**Total Effort:** 22–28 AI-hours (~150k tokens)  
**Risk:** Low – every phase ends with `cargo check --workspace` + smoke test  
**Branch recommendation:** `git checkout -b feat/v0.3-rig-workspace-wasi`

This plan is **exact, sequential, and self-verifying**. Follow it line-by-line. It respects **SRP**, **canonical naming**, **2024 edition**, and the v0.3 monorepo layout. No new features – only structural upgrades for maintainability and Rig-native reuse.

### Phase 0: Preparation (1 AI-hour)
1. Create branch (from clean v0.2 state):  
   ```bash
   git checkout -b feat/v0.3-rig-workspace-wasi
   git status  # ensure clean
   ```

2. Add WASI Preview 2 target (2026 standard):  
   ```bash
   rustup target add wasm32-wasip2
   ```

3. Verify current state:  
   ```bash
   cargo check --package agent-gateway
   ./start.sh infra  # or CONUSAI_TEST_MODE=1
   ```

**Validation:** `cargo tree | grep rig-core` should show v0.36. Commit: `"chore: v0.3 migration branch"`.

### Phase 1: Root Cargo Workspace Centralization (6–8 AI-hours)
**Goal:** Move workspace root to `conusai-platform/Cargo.toml` (v0.3 canonical layout).

**Step 1.1** – Create root `Cargo.toml` (new file at monorepo root):  
```toml
[workspace.package]
version = "0.3.0"
edition = "2024"
rust-version = "1.88"
authors = ["ConusAI Team"]

[workspace.dependencies]
# AI / LLM – Rig canonical
rig-core = "0.36"
rig-qdrant = "0.2"          # ← NEW: preferred VectorStoreIndex
# (copy ALL other deps from apps/backend/Cargo.toml exactly as they exist today)
tokio = { version = "1", features = ["full"] }
# ... (all crates listed in v0.2 section 4)

[workspace]
members = [
    "apps/backend/crates/common",
    "apps/backend/crates/agent-core",
    "apps/backend/crates/agent-gateway",
    "apps/backend/evals",
]
resolver = "3"
```

**Step 1.2** – Update `apps/backend/Cargo.toml`:  
- Delete the entire `[workspace]` section.  
- Keep only the package definition if any (or remove if it was purely workspace).  
- Add at top: `workspace = { version = "0.3.0" }` (inherits from root).

**Step 1.3** – For **every** `crates/*/Cargo.toml` and `evals/Cargo.toml`:  
- Replace every direct dependency version with workspace inheritance:  
  ```toml
  rig-core = { workspace = true }
  rig-qdrant = { workspace = true }
  # etc. for ALL crates listed in [workspace.dependencies]
  ```
- Ensure `edition = "2024"` and `rust-version = "1.88"`.

**Step 1.4** – Update import paths if any absolute paths broke (rare).  
**Validation command (run after every sub-step):**  
```bash
cargo check --workspace
```
**Expected:** Zero errors. All crates now resolve from root.

**Commit:** `"refactor: move Cargo workspace to monorepo root (v0.3 layout)"`

### Phase 2: Dependency Updates & Rig-qdrant Addition (1–2 AI-hours)
1. In root `Cargo.toml` `[workspace.dependencies]`, add/confirm:  
   ```toml
   rig-qdrant = "0.2"
   wasmtime = "44"
   wasmtime-wasi = "44"
   ```

2. Run:  
   ```bash
   cargo update
   cargo check --workspace
   ```

**Commit:** `"chore: add rig-qdrant 0.2 + wasmtime 44 (v0.3 deps)"`

### Phase 3: Migrate to Rig-native Vector Store (8–10 AI-hours) – Highest Impact
**Goal:** Delete custom Qdrant code; use `rig_qdrant::QdrantVectorStore` (implements `rig::VectorStoreIndex`).

**Step 3.1** – Delete obsolete file:  
   ```bash
   rm crates/agent-core/src/memory/qdrant_helpers.rs
   ```
   Remove from `mod.rs` and `lib.rs` re-exports.

**Step 3.2** – Refactor memory stores (`crates/agent-core/src/memory/`):  
   - `qdrant_store.rs` → `QdrantThreadStore` now wraps `rig_qdrant::QdrantVectorStore`.  
   - `qdrant_workspace_store.rs` → same.  
   - Use `rig::VectorStoreIndex` trait everywhere (semantic search, upsert, filter).  
   - Replace 4-dim zero vectors + SHA-256 hack with native Rig embeddings (64-dim for capabilities).  
   - `CapabilityCard` embedding now calls `index.upsert(...)` directly.

**Step 3.3** – Update `ContextBuilder` and capability search routes:  
   - Make generic over any type implementing `rig::VectorStoreIndex`.  
   - Semantic capability search now uses Rig’s vector search (no fallback needed in happy path).

**Step 3.4** – `agent-core/src/agent/builder.rs`:  
   Add `with_vector_store` method that forwards to underlying `rig::Agent`.

**Step 3.5** – Remove raw `reqwest` Qdrant calls from `http_client.rs` if only used for Qdrant.

**Validation:**  
```bash
cargo test --package agent-core --test qdrant  # or full workspace
CONUSAI_TEST_MODE=1 cargo run --bin agent-gateway --features test-mode
```
Test `/v1/capabilities/search` and thread/workspace persistence.

**Commit:** `"feat: migrate to rig-qdrant 0.2 (remove custom helpers)"`

### Phase 4: WASM Upgrade to wasip2 + wasmtime 44 (3–4 AI-hours)
1. Update `rust-toolchain.toml` (root or `apps/backend/`):  
   ```toml
   targets = ["wasm32-wasip2"]
   ```

2. Update `WasmLoader` / `WasmToolLoader` in `agent-core/src/tools/wasm_loader.rs`:  
   - Switch to Component Model APIs (`wasmtime::component::*`).  
   - Update `new_store` and `invoke_tool` to Preview 2 linking.

3. Recompile example capability:  
   ```bash
   cargo build --target wasm32-wasip2 -p template-wasm  # or whatever the wasm crate is
   ```

4. Update `capabilities/template-wasm/Cargo.toml` target if needed.

**Validation:**  
   - `cargo build --target wasm32-wasip2 --package agent-core`  
   - Test WASM `ping` tool via `/admin/capabilities/test` or UI.

**Commit:** `"refactor: upgrade to wasm32-wasip2 + wasmtime 44"`

### Phase 5: Documentation, CI, Docker & Polish (2 AI-hours)
1. Update `docs/arch.md`:  
   - Change version header to **v0.3**.  
   - Update Technology Stack table (wasmtime 44, rig-qdrant 0.2, wasm32-wasip2).  
   - Update Repository Layout diagram.  
   - Add new “Rig Integration Guideline” section from v0.3 instructions.  
   - Update Capability kinds table if needed.

2. Update:
   - `Dockerfile` (cargo-chef still works).
   - `docker-compose.yml` healthchecks (no change needed).
   - `.github/workflows/` CI (add `cargo check --workspace`).
   - `Makefile` / `start.sh` (update any cargo commands to root).
   - Any remaining `apps/backend/Cargo.toml` references.

3. Minor cleanups (SRP):
   - Ensure `Agent` cleanly wraps `rig::Agent`.
   - Remove any dead `qdrant_helpers` imports.

**Commit:** `"docs: update arch.md to v0.3 + CI/Docker polish"`

### Phase 6: Full Validation & Release (2–3 AI-hours)
Run in order:
```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
CONUSAI_TEST_MODE=1 cargo run --bin agent-gateway &
# manual smoke tests:
# - ./start.sh full
# - UI login + workspace + invoice extraction
# - /admin/capabilities/reload
# - WASM tool invocation
# - Semantic search
# - Agent completions with tools
```

**Final commit:** `"release: v0.3.0 – root workspace, rig-qdrant, wasip2"`


