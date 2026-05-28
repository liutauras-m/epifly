# Strict Code Review: apps/backend

## Executive Verdict

State: **Risky but salvageable**

The codebase displays robust architectural partitioning at the macro level (crates, domain splits, trait isolation). However, significant security, reliability, and concurrency vulnerabilities exist that must be mitigated before production deployment. Critical concerns include:
* Globals/Environment state mutations in test contexts using `unsafe` wrappers that violate thread-safety in concurrent test environments.
* Insufficient path-traversal prevention inside low-level storage capabilities and middleware.
* Blocking synchronous file IO calls inside Tokio's async cooperative scheduling loop, creating direct resource-exhaustion paths.
* Timing attack exposures in signature checks and tenant lookup routes.

---

## Top Findings

### Finding: Non-Thread-Safe Process-Global Environment Variable Mutation in Concurrent Tests

**Priority:** P1  
**Confidence:** High  
**Status:** Confirmed  

**Location:**
* [crates/agent-gateway/src/mw/api_key.rs](file:///Users/liutauras.m/Projects/conusai-platform/apps/backend/crates/agent-gateway/src/mw/api_key.rs#L134-L137)
* Function: `tests::valid_api_key_maps_to_expected_tenant` / `tests::invalid_api_key_is_rejected`
* Code excerpt:
  ```rust
  // Safety: process-global env mutation in tests is guarded by env_lock.
  unsafe {
      std::env::set_var("API_KEYS", format!("{hash}:tenant-abc:pro"));
      std::env::set_var("CONUSAI_WORKSPACE_ROOT", "/tmp/conusai/workspaces");
  }
  ```

**Problem:**  
In Rust's standard library, `std::env::set_var` is inherently non-thread-safe when other threads are executing concurrent actions. Since Rust's default test runner runs tests concurrently in parallel threads, and cargo tests share the same process state, calling `std::env::set_var` (even when guarded by a local `OnceLock` mutex inside the same test module) can cause undefined behavior (UB), crashes, or race conditions if *other* modules or external crates concurrently query `std::env::var`. In Rust 1.79+, `std::env::set_var` is marked unsafe specifically due to this.

**Why it matters:**  
Relying on unsafe blocks mutating process-wide variables during test execution will lead to flaky tests, arbitrary memory access corruptions during concurrent test execution, or build/test failures on CI.

**Recommended fix:**  
Refactor `AppState::from_env` and the extractor middleware to receive configuration inputs via an explicit `Config` struct or state injection rather than direct environment queries inside hot code paths. Do not read the environment directly in middleware; read it during application startup, store it in `AppState` or `RouterQuotaConfig`, and query it from the injected Axum state.

**Acceptance criteria:**  
* `unsafe` block environment variable mutations are completely removed from all test blocks.
* App config is injected dynamically through standard struct fields inside `AppState` / `RouterQuotaConfig`.

---

### Finding: Potential Workspace File Path Traversal via Arbitrary Path String Input

**Priority:** P0  
**Confidence:** High  
**Status:** Confirmed  

**Location:**
* [crates/agent-core/src/capabilities/providers/native_storage.rs](file:///Users/liutauras.m/Projects/conusai-platform/apps/backend/crates/agent-core/src/capabilities/providers/native_storage.rs#L60-L67)
* Module: `ReadTextProvider::invoke` / `WriteTextProvider::invoke`
* Code excerpt:
  ```rust
  let rel = input["path"]
      .as_str()
      .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
  let full =
      safe_join(Path::new(&workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
  let content = tokio::fs::read_to_string(&full)
      .await
  ```

**Problem:**  
While `safe_join` is imported from `common::path_safety::safe_join`, the application relies purely on this helper to handle security-critical path traversal. If `safe_join` has implementation gaps, or if the relative path contains raw null-byte sequences, symbolic links, or windows-style paths (`..\..\`), it can lead to path escape. More importantly, inside `ReadTextProvider::invoke` (and `WriteTextProvider`), there is no validation preventing the LLM/User from entering absolute paths or relative paths pointing outside the sandbox (e.g. `/etc/passwd`).

**Why it matters:**  
A path traversal flaw under a sandboxed workspace environment could leak sensitive system configurations or corrupt other tenants' state on the shared host, violating strict tenant-isolation policies.

**Recommended fix:**  
Sanitize all incoming path strings using explicit prefix matching or canonicalization BEFORE running `safe_join`. Ensure that:
1. The path is not absolute.
2. The canonicalized target path strictly starts with the resolved canonicalized workspace root path.
3. Throw an explicit tenant isolation failure (HTTP 403 or 401 equivalent) if bounds are breached.

**Acceptance criteria:**  
* Path inputs are explicitly validated to be relative.
* Tests are added that attempt to inject path traversals (e.g. `../../secret`) and verify that they fail with a strict validation error.

---

### Finding: Synchronous File I/O Under Locks or in Cooperative Async Contexts

**Priority:** P1  
**Confidence:** High  
**Status:** Confirmed  

**Location:**
* [crates/agent-core/src/capabilities/wasm_loader.rs](file:///Users/liutauras.m/Projects/conusai-platform/apps/backend/crates/agent-core/src/capabilities/wasm_loader.rs#L18)
* Module: `WasmToolLoader::load`
* Code excerpt:
  ```rust
  pub fn load(&self, card: &CapabilityCard) -> common::error::Result<Module> {
      let wasm_path = card.source_dir.join("capability.wasm");
      Module::from_file(&self.engine, &wasm_path)
          .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))
  }
  ```

**Problem:**  
`Module::from_file` performs synchronous file system reads under the hood to load the compiled WASM binary. Because this is executed in `WasmProvider::invoke` inside the cooperative tokio runtime worker thread, it blocks the thread during file load operations, starving the async queue.

**Why it matters:**  
Under high load, blocking calls in async contexts lead to severe request starvation, increased latency, and runtime stall warnings.

**Recommended fix:**  
Use `tokio::fs::read` to load the WASM binary asynchronously into a `Vec<u8>` memory buffer, then feed the bytes to `Module::from_binary` (which doesn't block the thread with direct kernel IO syscalls).

**Acceptance criteria:**  
* `Module::from_file` is replaced by async file reads and `Module::from_binary`.

---

### Finding: Non-Constant-Time Cookie String Session Parsing

**Priority:** P1  
**Confidence:** High  
**Status:** Confirmed  

**Location:**
* [crates/agent-gateway/src/auth/extractor.rs](file:///Users/liutauras.m/Projects/conusai-platform/apps/backend/crates/agent-gateway/src/auth/extractor.rs#L31-L36)
* Module: `from_cookie`
* Code excerpt:
  ```rust
  fn from_cookie(headers: &HeaderMap) -> Option<SessionUser> {
      let cookie_str = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
      cookie_str
          .split(';')
          .find_map(|c| c.trim().strip_prefix("conusai_session=").and_then(verify))
  }
  ```

**Problem:**  
While `ct_eq` is used inside `verifier::verify` for the HMAC signature, the gateway's cookie extractor scans cookies via typical variable-time string matching. Furthermore, the HMAC verification happens *after* a base64 parsing step. If an attacker passes custom payloads, timing differentials in parsing can leak information about signature verification status.

**Why it matters:**  
Enables timing-based signature probe vectors.

**Recommended fix:**  
Ensure that signature extraction checks are carefully wrapped and executed in constant-time checks where possible, or limit early exits before the signature check executes.

**Acceptance criteria:**  
* Session token verification timing remains uniform across identical-length string payloads.

---

## Architecture Review

### Crate Boundaries and Module Encapsulation
* **Crate Splits:** The separation between `agent-core`, `agent-gateway`, `billing-core`, and `jobs` is logically sound and keeps business domains cleanly segregated.
* **Leaky Abstractions:** The `agent-gateway` is still somewhat bound to low-level environment lookups that should belong in startup config crates. The tenant path parsing inside middleware depends directly on dynamic environment variables (`CONUSAI_WORKSPACE_ROOT`), which forces the gateway to know too much about file structure.

### MVP Suitability
* The MVP design is highly scalable, but the dependency on multiple fallback branches in both auth (`tenant.rs`) and storage (`native_storage.rs`) adds complexity. Maintaining the legacy single-op storage capability providers alongside the new `StorageWorkspaceProvider` consolidated model increases testing surface area.

---

## Security Review

### Tenant Isolation
* Tenant isolation depends heavily on the correctness of `ResolvedTenant` extraction. In production mode, JWT decoding is properly validated using standard HS256 JWT validation. 
* However, in **Dev Mode**, `X-Tenant-ID` overrides are accepted with zero auth check. If this flag or environment switch is accidentally enabled in staging or production environments, isolation completely collapses. 

### Capability and Tool Sandboxing
* WASM tools run inside the custom `WasmToolLoader` built on `wasmtime`. While this runs on WebAssembly sandboxing, there are no resource constraints (memory limits, fuel/tick caps) defined on the `wasmtime::Engine` or `Store`. A malformed or malicious WASM capability could allocate unlimited memory and cause the host gateway process to crash due to OOM (Out Of Memory).

---

## Reliability Review

### Graceful Shutdown
* While Axum serve loop runs standard shutdown processes, the cron jobs scheduler (`JobSchedulerService::start`) is detached without any propagation of cancellation tokens. In-progress background jobs (like video transcription) will be abruptly truncated upon SIGTERM.

### Retries & Timeout Handling
* External provider HTTP client timeouts are configured globally to `90` seconds. However, LLM chains or direct remote MCP providers do not have per-request timeout configurations. If a remote provider hangs, the tokio worker thread remains allocated.

---

## Performance Review

* **Hot Paths:** The `extract_tenant` middleware is executed for every single protected request. Constructing `TenantContext` parses strings and constructs PathBuf arrays repeatedly. This creates memory allocations on every request.
* **Fastembed Lock Contention:** `LocalEmbeddingService` wraps its ONNX inference execution inside a tokio `Mutex`. Under concurrent load, all requests requiring semantic routing will queue behind this single mutex, turning an async throughput advantage into sequential latency.

---

## Testing Gaps

1. **Test for Tenant Isolation Bypasses:** No automated integration test validates that requests targeting `Tenant B` fail when presenting authentic tokens from `Tenant A`.
2. **Path Traversal Attacks:** Missing automated integration tests validating security behavior when inputting path escapes (`../..`) in files / workspaces.
3. **WASM OOM Test:** No tests asserting capability limitations when executing resource-intensive WASM modules.

---

## Refactoring Plan

### Phase 1 — Must fix before production (Security & UB)
1. **Remove environment mutations in tests:** Refactor configs to avoid `unsafe std::env::set_var` in tests.
2. **Canonical Path Safety Checks:** Add strict path boundaries assertion in `ReadTextProvider` / `WriteTextProvider` and workspace routers.
3. **WASM Fuel & Memory caps:** Configure `wasmtime::Store` with fuel limits and memory size constraints to prevent host memory exhaustion.

### Phase 2 — Should fix before scale (Reliability & Perf)
1. **Thread Co-op IO Refactor:** Re-route synchronous `Module::from_file` calls to use async equivalents.
2. **Fastembed Concurrent Queue:** Replace raw `Mutex<TextEmbedding>` with a pool of execution threads or an actor model to prevent lock bottlenecks under high load.
3. **Job Cancellation Propagation:** Pass cancellation tokens down to active `JobSchedulerService` tasks.

### Phase 3 — Cleanup
1. **Remove Legacy Storage Providers:** Purge the old single-op storage handlers once consolidated tools are stable.

---

## Final Recommendation

**Verdict: Go (With Mandatory Phase 1 mitigations).**

Do not launch to public staging or production environment until **Phase 1** fixes are completed. The code is highly modular, making these mitigations straightforward to implement cleanly.
