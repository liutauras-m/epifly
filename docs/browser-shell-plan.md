# ConusAI Browser Shell — Implementation Plan v1.1

> **Source.** Reconciles [docs/tasks/browser-shell-task.md](tasks/browser-shell-task.md)
> against the live workspace state on 2026-05-10 and extends it to **all four
> targets the user asked for**: macOS, iOS, Windows, Android.
>
> **v1.1 changelog (this revision).** Folds in the v1.1 review with selective
> acceptance:
> - **ACCEPTED:** rename `BrowserShellReplayCapability` → `TraceReplayCapability`
>   (Browser is an implementation detail of one possible trace source); move
>   `SessionTrace` + `UserStep` into `crates/common/src/trace.rs` as the
>   versioned source of truth; introduce two thin traits (`SessionRecorder`,
>   `TraceSource`) at the `agent-core` boundary.
> - **REJECTED with rationale (§8):** extracting `recorder`, `tabs`, and
>   `telemetry` into three separate Tauri plugin crates *before* a second
>   consumer exists; building a generic `DeviceHub` abstraction; making
>   capability auto-registration dynamic. All three are YAGNI for one shell
>   today — they violate "only changes directly requested or clearly
>   necessary". Trait boundaries (above) preserve the option to extract later
>   at zero cost.
>
> **Stack baseline (locked in).** Tauri 2.x · SvelteKit 2 + Svelte 5 runes ·
> pnpm 10 (already pinned in [package.json](../package.json)) · Vite 6 ·
> TypeScript 5.7 · Cargo workspace edition 2024 · Wry → WKWebView (Apple),
> WebView2 (Windows), WebView (Android system).
>
> **Reuses existing backend symbols (do NOT re-create).**
> - `CapabilityProvider` ([provider.rs](../apps/backend/crates/agent-core/src/tools/provider.rs))
> - `CapabilityCard` ([card.rs](../apps/backend/crates/agent-core/src/tools/card.rs))
> - `ToolRegistry` ([registry.rs](../apps/backend/crates/agent-core/src/tools/registry.rs)) — see §0.2 on the deferred rename
> - `POST /admin/capabilities/register` ([admin_capabilities.rs](../apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs))
> - `ArtifactBridge::process_if_artifacts` ([artifact_bridge.rs](../apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs))
> - `POST /v1/files`, `POST /v1/workspaces`, `GET /api/realtime/workspace`
> - `DynamicPromptCapability` ([dynamic_prompt.rs](../apps/backend/crates/agent-core/src/chains/dynamic_prompt.rs)) — inner orchestrator for `TraceReplayCapability` (NOT the task doc's mythical `PromptChainCapability`)

---

## 0. Workspace audit (what exists today vs the task's assumptions)

| Task doc claim | Reality on `main` (2026-05-10) | Action |
|---|---|---|
| `apps/web` is missing | Already exists ([apps/web](../apps/web/)) with SvelteKit 2 + Svelte 5, adapter-node, vitest | **Refactor, do not scaffold** |
| `pnpm-workspace.yaml` is missing | Exists, includes only `apps/web` | Extend to `packages/*` and `apps/browser-shell` |
| pnpm 9 | pnpm **10.13.1** is pinned | Use 10; ignore the task's "9" |
| `packages/` is missing | Confirmed missing | Create per Phase 2 |
| `turbo.json`, `biome.json`, `justfile`, `.tool-versions` missing | Confirmed missing | Create per Phase 1 |
| `PromptChainCapability` exists | Only `DynamicPromptCapability` + `chains/llm_chain.rs` | Use `DynamicPromptCapability` as the inner orchestrator |
| `device_tokens` table | Does **not** exist | Add migration in Phase 5 |
| Rust edition 2021 | Workspace is **edition 2024** ([Cargo.toml](../Cargo.toml)) | Tauri crate must match |
| `rust-toolchain.toml` | Does **not** exist | Create with `1.95+` per task |
| Workspace version | `0.3.1` ([Cargo.toml](../Cargo.toml#L19)) | Bump to `0.4.0` only at Phase 7 ship |

### 0.1 Endpoints already in place (verified)

`workspaces.rs`, `files.rs`, `realtime.rs`, `admin_capabilities.rs`,
`capabilities.rs`, `search.rs`, `agent.rs`, `auth.rs`, `mcp.rs`, `chat.rs`,
`threads.rs`, `tasks.rs`, `audit.rs`, `health.rs` are all wired in
[routes/mod.rs](../apps/backend/crates/agent-gateway/src/routes/mod.rs).

### 0.2 The rename decision (`ToolRegistry` → `CapabilityRegistry`)

The task doc mandates this rename in **Phase 0.5** as a precondition. The
[capability gaps plan §4](capability-gaps-pan.md) **deferred** it as scope
creep against the operational hardening epic.

**Resolution for this plan:** the rename is a *prerequisite for vocabulary
clarity in Phase 4-5 docs and the Tauri Rust glue*, but it remains a separate
PR with its own gate. Sequencing:

1. Capability gap-plan PR-1/2/3 ship first (already committed: `0834d26`,
   `749bb96`).
2. **Standalone rename PR (this plan's Phase 0.5)** — single behaviour-preserving commit.
3. Browser-shell phases proceed against the renamed surface.

If the rename is rejected at review, every reference below to
`CapabilityRegistry` reads as `ToolRegistry`. No other change required.

---

## 1. Target monorepo layout (end-state)

```
conusai-platform/
├── apps/
│   ├── backend/                      # unchanged shape
│   ├── web/                          # existing SvelteKit (adapter-node) — refactored to consume packages/*
│   └── browser-shell/                # NEW — Tauri 2 shell (mac/ios/win/android)
│       ├── src/                      #   SvelteKit (adapter-static) — reuses packages/ui
│       └── src-tauri/                #   Rust crate — added to root Cargo workspace
│           ├── src/
│           ├── capabilities/         #   per-platform main-capability.json
│           ├── gen/{apple,android}/  #   generated by tauri ios/android init
│           └── tauri.conf.json
├── packages/
│   ├── ui/                           # NEW — Svelte 5 components, Tailwind 4
│   ├── types/                        # NEW — typed OpenAPI + valibot schemas
│   └── sdk/                          # NEW — @conusai/sdk typed client
├── docs/                             # unchanged (this plan lives here)
├── docker/, services/                # unchanged
├── Cargo.toml                        # extend [workspace.members]
├── package.json                      # already exists (pnpm 10)
├── pnpm-workspace.yaml               # extend
├── turbo.json                        # NEW
├── biome.json                        # NEW
├── justfile                          # NEW
├── .tool-versions                    # NEW
└── rust-toolchain.toml               # NEW
```

---

## 2. Phased plan — strict ordering

Each phase ends with a hard gate:
`cargo check --workspace && cargo clippy --workspace -- -D warnings && pnpm -w build && pnpm -w test`.
After every UI-facing phase, run the **plan-browser-verifier** skill on a
real browser/Tauri build.

> **Effort scale.** Units are relative weights (1 unit ≈ a half-day of
> focused work). Total ≈ 30 units across 8 phases.

---

### Phase 0 — Branch, ADRs, prerequisites — **1 unit**

**Goal:** capture irreversible architectural decisions before any code lands.

**Steps**
1. `git checkout -b feat/browser-shell` from `main` (HEAD `0834d26`+).
2. Write three ADRs:
   - `docs/adr/006-tauri-browser-shell.md` — **Tauri 2 vs Electron** (binary size, Rust reuse), **SvelteKit vs Next.js** (Svelte 5 runes, smaller bundle), **adapter-node for web vs adapter-static for shell**, **why Askama UI stays in parallel** during transition.
   - `docs/adr/007-capability-module-rename.md` — record the `ToolRegistry` → `CapabilityRegistry` rename rationale (vocabulary alignment) and the deprecation aliases policy (one release).
   - `docs/adr/008-multi-platform-shell.md` — single `src-tauri/` for **macOS + Windows + iOS + Android**, justify Tauri 2 mobile maturity, document per-target build hosts (macOS for Apple, any for Win/Linux/Android).
3. Create tooling pin files (no global installs in CI):
   - `.tool-versions`: `nodejs 22.12.0`, `pnpm 10.13.1`, `rust 1.95.0`, `just 1.36.0`.
   - `rust-toolchain.toml`: `[toolchain] channel = "1.95"`, `targets = ["aarch64-apple-darwin","x86_64-apple-darwin","aarch64-apple-ios","aarch64-apple-ios-sim","aarch64-linux-android","armv7-linux-androideabi","x86_64-pc-windows-msvc"]`.
4. Update root `README.md` with a one-paragraph "Cross-platform shell" section pointing here.

**Gate.** ADRs reviewed; `rustup show` lists all five targets locally.

---

### Phase 0.5 — Canonical capability rename — **1–2 units, single PR**

**Goal:** behaviour-preserving rename. Must merge before Phase 4.

**Steps**
1. `git mv apps/backend/crates/agent-core/src/tools apps/backend/crates/agent-core/src/capabilities`.
2. Update every `mod tools;` / `use crate::tools::…` → `mod capabilities;` / `use crate::capabilities::…`. Use `cargo fix --workspace` + IDE "Rename Symbol" for `ToolRegistry` → `CapabilityRegistry`.
3. Add backward-compatible re-exports in [agent-core/src/lib.rs](../apps/backend/crates/agent-core/src/lib.rs):
   ```rust
   #[deprecated(note = "use capabilities::CapabilityRegistry")]
   pub use capabilities::CapabilityRegistry as ToolRegistry;
   #[deprecated(note = "use capabilities")] pub use capabilities as tools;
   ```
4. Update doc-comments in `agent-gateway/src/state.rs` (3 sites), `chains/llm_chain.rs`, `bridge/artifact_bridge.rs`, `evals/`, `jobs/`.
5. Run `cargo fmt --all && cargo clippy --workspace -- -D warnings && cargo test --workspace`.
6. Refresh [memories/repo/dynamic-tool-registration.md](../memories/repo/dynamic-tool-registration.md) with the new names.

**Gate.** Existing eval suite (`cargo run -p evals`) produces byte-identical
report. Zero behavioural diff.

---

### Phase 1 — Frontend monorepo skeleton — **1–2 units**

**Goal:** establish workspace tooling so subsequent packages slot in.

**Steps**
1. Extend [pnpm-workspace.yaml](../pnpm-workspace.yaml):
   ```yaml
   packages:
     - "apps/web"
     - "apps/browser-shell"
     - "packages/*"
   ```
2. Add `turbo.json` with pipelines `build`, `dev` (persistent), `lint`, `test`, `check-types`. Cache outputs: `.svelte-kit/`, `dist/`, `build/`, `src-tauri/target/`.
3. Add `biome.json` (replaces ad-hoc ESLint/Prettier — 2026 default). Configure import-sort, Svelte plugin, organize-imports on save.
4. Add root `justfile` with discoverable recipes:
   ```just
   default:        @just --list
   web-dev:        pnpm --filter web dev
   shell-dev:      pnpm --filter browser-shell tauri dev
   shell-build:    pnpm --filter browser-shell tauri build
   types:          ./scripts/openapi-to-types.sh
   verify:         cargo clippy --workspace -- -D warnings && pnpm -w lint && pnpm -w test && cargo test --workspace
   ```
5. Extend `.gitignore`: `node_modules/`, `.turbo/`, `.svelte-kit/`, `apps/*/build`, `apps/*/dist`, `apps/browser-shell/src-tauri/target/`, `apps/browser-shell/src-tauri/gen/`.
6. Extend [Cargo.toml](../Cargo.toml) `[workspace.members]` with `apps/browser-shell/src-tauri` (Phase 4 will create the crate).

**Gate.** `pnpm install` clean; `cargo metadata` resolves; `just verify` green.

---

### Phase 2 — Shared packages — **2–3 units**

#### 2.1 `packages/types`
**Steps**
1. Create `packages/types/package.json` (`@conusai/types`, type=module, no runtime deps).
2. Add `scripts/openapi-to-types.sh`: boots gateway in test mode (`CONUSAI_TEST_MODE=1`), runs `pnpm openapi-typescript http://localhost:8088/api-docs/openapi.json -o packages/types/src/openapi.d.ts`, kills server.
3. Add `scripts/openapi-to-valibot.ts`: parses the OpenAPI JSON, emits **Valibot** schemas (1 kB tree-shaken, 2026 successor to Zod) to `packages/types/src/valibot.ts`.
4. Hand-written domain types in `packages/types/src/domain.ts` mirroring [common/src/artifact.rs](../apps/backend/crates/common/src/artifact.rs) and `WorkspaceNode`: `SessionTrace`, `UserStep`, `CapabilityCard`, `ControlMessage`.
5. CI guard: `just types` in CI fails the build if `git diff --exit-code packages/types/src` is non-empty (drift detection).

#### 2.2 `packages/sdk` (`@conusai/sdk`)
**Steps**
1. Native `fetch`-based client; one method per real endpoint:
   - `auth.login`, `auth.logout`
   - `capabilities.list`, `capabilities.search(q, limit)`, `capabilities.register(manifest)`
   - `files.upload(file: File): Promise<FileToken>`
   - `workspaces.create(node)`, `workspaces.tree(parentId?)`, `workspaces.get(id)`
   - `realtime.subscribe(): WebSocket` (auto-reconnect, jittered backoff 0.5s→30s)
   - `shells.control(deviceId): WebSocket` (Phase 5 endpoint)
2. Token storage via injected `tokenProvider: () => Promise<string>`. **Never localStorage.**
3. Retry: exponential backoff on 5xx + idempotent verbs (GET, PUT) only. POST/PATCH never retry.
4. 100% test coverage with `vitest` + `msw` mocked HTTP.
5. Reuse this SDK from `apps/web` (replace inline `fetch` in [apps/web/src/lib/api/](../apps/web/src/lib/api/)).

#### 2.3 `packages/ui`
**Steps**
1. Svelte 5 (runes-only) component library + Tailwind 4 (oxide engine).
2. Tokens from [docs/ui-design.md](ui-design.md): warm neutrals, single teal accent, Archivo + Inter + Space Mono.
3. Components needed for both web and shell parity:
   - `AppShell`, `CommandPalette`, `CapabilityCard`, `WorkspaceTree`, `RecorderControls`, `TabStrip`, `ToastHost`, `ArtifactPreview`.
4. All components import from `@conusai/types`, never from app code.
5. Histoire (lighter than Storybook for Svelte 5) with one story per component.
6. **a11y CI**: `axe-core` runs against every story; zero serious/critical violations.

**Gate.** `pnpm -w build`, `pnpm -w test`, `pnpm --filter ui histoire build`.

---

### Phase 3 — Refactor `apps/web` onto packages — **2 units** (down from 3 because the app exists)

**Goal:** `apps/web` consumes only `@conusai/{ui,sdk,types}`; no inline fetch.

**Steps**
1. Add `@conusai/{ui,sdk,types}` as workspace deps in [apps/web/package.json](../apps/web/package.json) via `workspace:*`.
2. Replace [apps/web/src/lib/api/](../apps/web/src/lib/api/) call sites with `@conusai/sdk`. Delete the in-app type duplicates that now live in `@conusai/types`.
3. Move the existing components in [apps/web/src/lib/](../apps/web/src/lib/) that are reusable (workspace tree, toast host, command palette) into `packages/ui` — keep app-specific (page layout, route components) in `apps/web`.
4. Add server-side CSP via SvelteKit `handle` hook in [apps/web/src/hooks.server.ts](../apps/web/src/hooks.server.ts):
   `default-src 'self'; connect-src 'self' wss:; img-src 'self' data: blob:; script-src 'self'; style-src 'self' 'unsafe-inline'`.
5. Auth: keep the existing HMAC `conusai_session` cookie ([apps/web/src/lib/server/](../apps/web/src/lib/server/)); add CSRF token via SvelteKit form actions.
6. Telemetry: add `@vercel/otel` browser SDK exporting to the existing OTLP collector (`http://localhost:4318` dev, configurable via `PUBLIC_OTLP_URL`).

**Gate.** Lighthouse ≥ 95 on `/`. Existing vitest suite still green. Playwright e2e: login → create workspace → upload file → file appears in tree.

---

### Phase 4 — `apps/browser-shell` desktop scaffold — **5–7 units**

This is the heaviest phase. Ship a working slice every sub-step.

#### 4.1 Scaffold (no destructive choices yet)
```bash
cd apps && pnpm create tauri-app@latest browser-shell \
  --template svelte-ts --manager pnpm
```
Then:
1. Replace `adapter-auto` with `@sveltejs/adapter-static` (`fallback: 'index.html'`) so `pnpm build` produces a bundleable static site.
2. Set `frontendDist = "../build"` in `src-tauri/tauri.conf.json`.
3. Add workspace deps `@conusai/{ui,sdk,types}`.
4. Confirm `apps/browser-shell/src-tauri/Cargo.toml` is detected as a workspace member by `cargo metadata`.

#### 4.2 Tauri config (cross-platform single source)
`src-tauri/tauri.conf.json`:
```jsonc
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ConusAI Browser",
  "version": "0.4.0",
  "identifier": "com.conusai.browser",
  "build": {
    "frontendDist": "../build",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm --filter browser-shell dev",
    "beforeBuildCommand": "pnpm --filter browser-shell build"
  },
  "app": {
    "withGlobalTauri": false,
    "security": {
      "csp": "default-src 'self'; connect-src 'self' https: wss:; img-src 'self' data: blob:"
    },
    "windows": [{ "title": "ConusAI Browser", "width": 1280, "height": 800 }]
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg", "msi", "appimage", "deb"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

#### 4.3 Capabilities allowlist (Tauri 2 permissions, **no wildcards**)
Create `src-tauri/capabilities/main-capability.json`:
- `core:default`
- `core:webview:allow-create-webview-window`
- `core:event:default`
- `dialog:default`
- `fs:scope-app-data`
- `http:default` scoped to `https://api.conusai.com` and dev `http://localhost:8080`
- `stronghold:default` (token vault)
- DEV ONLY (gated by `cfg(debug_assertions)`): `webview:allow-internal-toggle-devtools`

**Forbidden:** `dangerousRemoteDomainIpcAccess`, wildcard scopes, broad `fs:*`.

#### 4.4 Multi-tab WebView (`src-tauri/src/tabs.rs`)
- `Tabs` struct holds `HashMap<TabId, WebviewWindow>`.
- Each `WebviewWindow` gets a unique `data_directory` (per-tab cookie/storage isolation).
- Tauri commands: `create_tab(url) -> TabId`, `close_tab(id)`, `navigate(id, url)`, `list_tabs() -> Vec<TabSummary>`.
- Persist tab list to Stronghold on close, restore on launch.

#### 4.5 Step recorder (`src-tauri/src/recorder.rs`)
- Defines a `SessionRecorder` trait in `crates/common/src/trace.rs` (so the
  same shape can later back a headless / mobile recorder without changing
  callers):
  ```rust
  pub trait SessionRecorder: Send + Sync + 'static {
      fn record_step(&self, step: UserStep);
      fn snapshot(&self) -> SessionTrace;
      fn reset(&self);
  }
  ```
- Tauri-side `Recorder` struct (one per tab) implements `SessionRecorder`.
- JS bridge injected via `WebviewBuilder::initialization_script` listens for `click`, `input`, `submit`, `navigation` and calls `__TAURI__.core.invoke('record_step', step)`.
- Selector strategy: `data-testid` → unique CSS path → XPath fallback.
- **PII filter list (hard-coded in `crates/common`, non-bypassable):** never record values from `<input type=password>`, `autocomplete=cc-*`, `name=ssn|social|cpf`, `aria-label` matching `/password|secret|token/i`. Server-side validation rejects any trace whose steps contain redacted-but-present values.
- Screenshots: throttled 1/sec, PNG → base64; password-input regions blurred via canvas before encode.
- Output: `SessionTrace { id: ULID, started_at, ended_at, steps: Vec<UserStep>, urls: Vec<String> }` — type defined once in [common/src/trace.rs](../apps/backend/crates/common/src/trace.rs).

#### 4.6 ArtifactBridge integration (zero new backend code)
On "Stop recording":
1. Serialize `SessionTrace` to JSON.
2. `POST /v1/files` (multipart) → `FileToken`.
3. `POST /v1/workspaces` `{ kind: "file", name: "session-<ulid>.trace.json", metadata: { source: "browser-shell", trace_id, urls } }`.
4. Existing `WorkspaceIndexer` picks it up; `ArtifactBridge::process_if_artifacts` materialises downstream.

#### 4.7 Auto-registration on launch (`src-tauri/src/main.rs`)
On first run, POST `/admin/capabilities/register` with:
```json
{
  "capability_id": "trace.replay",
  "kind": "remote_mcp",
  "endpoint": "ws://localhost:0/<unused>",
  "tools": [{
    "name": "replay_session",
    "description": "Replay a recorded SessionTrace as a deterministic plan",
    "input_schema": { "type": "object", "properties": { "trace_node_id": { "type": "string" }, "dry_run": { "type": "boolean" } }, "required": ["trace_node_id"] }
  }],
  "tenant_scope": []
}
```
Authenticated via the **per-install device token** stored in Stronghold (Phase 5
issues these via a new `/admin/devices` endpoint). Never use a super-admin JWT.

> **Why a static manifest, not dynamic discovery?** The v1.1 review proposed
> the shell query its own `CapabilityCard`s at launch and POST them. Rejected
> for now: the shell exposes exactly **one** capability today. Static JSON is
> 12 lines and trivially auditable; dynamic discovery is a build-time vs
> runtime trade-off worth taking only when the shell exposes ≥3 capabilities.
> Revisit at v0.4.1.

#### 4.8 `TraceReplayCapability` (Rust, in `agent-core`)
- File: `apps/backend/crates/agent-core/src/capabilities/trace_replay.rs`
  (post-Phase-0.5 path; reads `tools/` if rename is rejected).
- Lives in `agent-core`, **not** the gateway, so future trace producers
  (mobile, headless CI, desktop recorders) reuse it.
- Implements `CapabilityProvider`. **Composes the existing `DynamicPromptCapability`** as the inner LLM orchestrator (system prompt: *"Translate these steps into a deterministic replay plan…"*). No bespoke Rig glue.
- **Type signature:**
  ```rust
  pub struct TraceReplayCapability {
      inner: Arc<DynamicPromptCapability>,
      trace_source: Arc<dyn TraceSource>,
  }
  ```
- The `TraceSource` trait (defined alongside the struct) abstracts "where the
  trace came from". The only initial impl is `WorkspaceNodeTraceSource` that
  loads the trace JSON via `WorkspaceNodeRepo::get(trace_node_id)`. Adding a
  future source (e.g. uploaded file, S3 URL) is a 30-line impl.
- Phase 1 ships **dry-run only** — returns the plan as `ToolOutput.artifacts[0]` of type `application/json`. Live replay deferred to v0.4.1.
- Backward-compat: `#[deprecated(note = "use TraceReplayCapability")] pub use TraceReplayCapability as BrowserShellReplayCapability;` in `agent-core::lib` for one release.

#### 4.9 Shared UI + telemetry on the shell
- Reuse `AppShell`, `TabStrip`, `RecorderControls`, `WorkspaceTree`, `CommandPalette`, `ArtifactPreview` from `@conusai/ui`.
- Tauri `event` plugin for tab/recorder events.
- `tauri-plugin-opentelemetry` (or a ~50-line OTLP/HTTP bridge in `src-tauri/src/telemetry.rs` if upstream is too immature) — Rust spans flow to the same OTel collector at `:4318` with `tenant.id` / `session.id` / `capability.name` attributes. Traces stitch end-to-end with the gateway in Jaeger.

**Gate.** `cargo tauri build` produces a signed macOS `.app` (dev cert) and a Linux AppImage. Manual smoke: open a public test page (httpbin.org) → Record → click 3 elements → Stop → trace appears as a workspace node.

---

### Phase 4M — macOS native polish — **0–1 units** (mostly free)

macOS is the **default desktop target** of Phase 4 — no extra `tauri macos init`,
no extra crates. The same `src-tauri` produces universal `.app` + `.dmg`.

**Steps**
1. Add `--target universal-apple-darwin` to the release recipe:
   ```just
   shell-build-macos: pnpm --filter browser-shell tauri build -- --target universal-apple-darwin
   ```
2. Code-sign with Apple Developer ID:
   - Local: `security find-identity -v -p codesigning`; export `APPLE_SIGNING_IDENTITY`.
   - CI (`release-shell.yml`): import cert from GH OIDC-fronted vault; never raw secrets.
3. Notarize via `notarytool` in CI:
   ```bash
   xcrun notarytool submit ConusAI-Browser.dmg --keychain-profile "ConusAI" --wait
   ```
4. Hardened runtime + entitlements file `src-tauri/macos/entitlements.plist` (only `com.apple.security.network.client`).
5. Verify with `codesign -dv --verbose=4` and `spctl --assess --verbose=4`.

**Gate.** Notarized `.dmg` opens cleanly on a fresh macOS 14 VM; recorder works.

---

### Phase 4W — Windows MSI + signing — **1–2 units**

**Steps**
1. Build host: `windows-2022` GH runner; install WebView2 runtime is implicit (Windows 11 ships it).
2. Add target: `rustup target add x86_64-pc-windows-msvc`.
3. Cargo build: `cargo tauri build -- --target x86_64-pc-windows-msvc` → MSI in `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/`.
4. **Code-signing via Azure Trusted Signing** (2026 standard; replaces deprecated EV USB tokens):
   - Configure `signtool` with Azure-managed cert.
   - In `tauri.conf.json` → `bundle.windows.signCommand`: `signtool sign /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 /dlib AzureCodeSigning.dll /dmdf metadata.json %1`.
5. Add WebView2 fixed-version fallback in `bundle.windows.webviewInstallMode = "embedBootstrapper"` for Windows 10 LTSC compatibility.
6. Add justfile recipe:
   ```just
   shell-build-windows: pnpm --filter browser-shell tauri build -- --target x86_64-pc-windows-msvc
   ```
7. CI matrix: `release-shell.yml` adds `windows-2022` job; uploads signed MSI + portable `.exe` to GitHub Releases.

**Gate.** Signed MSI installs silently (`msiexec /i ConusAI-Browser.msi /quiet`) on a fresh Windows 11 VM; SmartScreen does **not** warn (signature trust path complete); recorder works.

---

### Phase 4i — iOS target — **2–3 units, macOS host required**

**Prerequisites (already satisfied by Phase 0 toolchain pins)**
- macOS 14+, Xcode 16+, Apple Developer account.
- `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`.

**Steps**
1. Initialize once: `cd apps/browser-shell && pnpm tauri ios init`. Creates `src-tauri/gen/apple/` (Xcode project) and merges iOS bundle id into `tauri.conf.json`.
2. Extend `tauri.conf.json` `bundle.iOS`:
   ```jsonc
   "iOS": {
     "developmentTeam": "ABCDE12345",
     "minimumSystemVersion": "16.0",
     "frameworks": [],
     "template": null
   }
   ```
3. **No code change** to `recorder.rs` — WKWebView supports `initialization_script`, `take_screenshot`, and the `__TAURI__.core.invoke` bridge identically.
4. **Mobile responsiveness pass on `packages/ui`**: Tailwind mobile-first utilities, viewport meta in `AppShell.svelte`, hit-target ≥ 44 pt for `RecorderControls`.
5. **iOS-specific permissions**: add `src-tauri/capabilities/ios-capability.json` with `--target iOS` annotations. **Drop** `webview:allow-internal-toggle-devtools` in release.
6. **Privacy manifest** (Apple now mandatory, 2024+): `src-tauri/gen/apple/PrivacyInfo.xcprivacy` declaring `NSPrivacyAccessedAPITypes` for screenshot APIs and explicit consent toggle in UI for recorder.
7. justfile:
   ```just
   shell-ios-dev:    pnpm --filter browser-shell tauri ios dev
   shell-ios-build:  pnpm --filter browser-shell tauri ios build --target aarch64
   ```
8. CI: extend `release-shell.yml` with `macos-14` job, matrix target `aarch64-apple-ios`. Provisioning profile + cert from GH OIDC vault. Output: `.ipa` artifact.

**Gate.** `just shell-ios-dev` launches in iPhone 16 Simulator; record 3 steps on a public page; trace appears in workspace tree. `tauri ios build` succeeds without signing for simulator.

---

### Phase 4A — Android target — **2–3 units, any host**

**Prerequisites**
- JDK 17+ (`brew install openjdk@17` / `apt install openjdk-17-jdk`).
- Android Studio Hedgehog 2024.1+ (or just SDK + NDK 27 via `sdkmanager`).
- `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`.
- Env: `ANDROID_HOME`, `NDK_HOME`, in `.tool-versions`.

**Steps**
1. Initialize once: `cd apps/browser-shell && pnpm tauri android init`. Creates `src-tauri/gen/android/` (Gradle project).
2. Extend `tauri.conf.json` `bundle.android`:
   ```jsonc
   "android": {
     "minSdkVersion": 26,
     "compileSdkVersion": 35,
     "versionCode": 1,
     "permissions": ["android.permission.INTERNET"]
   }
   ```
3. **WebView constraint:** Android system WebView ≥ 100 (Android 8+ from auto-update). Validate the JS injection bridge — Android WebView accepts `addJavascriptInterface` which Tauri 2 maps to the same `__TAURI__.core.invoke` surface.
4. **Recorder caveats on Android:**
   - Screenshots via `WebView.capturePicture()` — **deprecated**; use `View.draw(Canvas)` fallback in the Kotlin shim Tauri generates. Cap at 1/sec.
   - Hardware back button → `Recorder::pop_step()` semantics.
5. Justfile:
   ```just
   shell-android-dev:    pnpm --filter browser-shell tauri android dev
   shell-android-build:  pnpm --filter browser-shell tauri android build --apk
   shell-android-aab:    pnpm --filter browser-shell tauri android build --aab  # Play Store
   ```
6. **Code-signing**: keystore generated once via `keytool`, stored in GH OIDC vault. Configure `gradle.properties` to read `CONUSAI_KEYSTORE_*` from env.
7. CI: extend `release-shell.yml` with `ubuntu-24.04` job (Android builds don't need macOS); install JDK 17 + Android SDK via `android-actions/setup-android@v3`. Upload signed `.aab` + `.apk`.

**Gate.** `just shell-android-dev` opens app on Pixel 7 emulator; record 3 steps on a public page; trace appears in workspace tree. `tauri android build --aab` produces a signed AAB.

---

### Phase 5 — Backend wiring for shells — **2 units**

**Goal:** make the gateway aware of device-token-authenticated shells and add
the replay capability to the registry.

**Steps**
1. Define `SessionTrace`, `UserStep`, `SessionRecorder` trait, and the PII
   filter list **once** in [common/src/trace.rs](../apps/backend/crates/common/src/trace.rs).
   `agent-core`, `agent-gateway`, and the Tauri shell all import from here.
   `packages/types` mirrors the schema via `schemars` → JSON Schema →
   openapi-typescript.
2. Register `TraceReplayCapability` (from §4.8) via `BuiltinFactory` (or a new `TraceReplayFactory`) in `CapabilityRegistry::with_default_factories`. The `TraceSource` impl wired by default is `WorkspaceNodeTraceSource`.
3. New SQL migration `apps/backend/crates/common/migrations/20260515000000_device_tokens.up.sql`:
   ```sql
   CREATE TABLE device_tokens (
     id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
     tenant_id     TEXT NOT NULL,
     device_label  TEXT NOT NULL,
     token_hash    BYTEA NOT NULL UNIQUE,  -- blake3(token)
     created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
     last_seen     TIMESTAMPTZ,
     revoked_at    TIMESTAMPTZ
   );
   CREATE INDEX device_tokens_tenant_idx ON device_tokens(tenant_id) WHERE revoked_at IS NULL;
   ```
4. New endpoints in [admin_capabilities.rs](../apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs) (or a sibling `admin_devices.rs`):
   - `POST /admin/devices` — issues a fresh token (returns plaintext **once**); requires `PLATFORM_ADMIN_TOKEN` bearer.
   - `DELETE /admin/devices/{id}` — sets `revoked_at`.
   - `GET /admin/devices` — lists active devices for the tenant.
5. Extend `CapabilityRegisterRequest` to accept either `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>` (legacy) or `X-Device-Token: <plaintext>` (new). Lookup via `blake3(token)` against `device_tokens.token_hash`.
6. New WebSocket route `GET /v1/shells/{device_id}/control` — pure transport in the gateway, dispatches to a new `Arc<ShellControlHub>` injected through `AppState`. Quotas: `RouterQuotaConfig.max_replays_per_turn` (default 3, env `CONUSAI_MAX_REPLAYS_PER_TURN`).
7. **Feature flag**: gate every shell-only route (`/v1/shells/*`, `/admin/devices`, `TraceReplayCapability` registration) behind `CONUSAI_FEATURE_BROWSER_SHELL=1`. Default off until Phase 7 ships.
8. utoipa annotations on all new routes; regenerate `packages/types/src/openapi.d.ts` via `just types`.
9. Tests in `apps/backend/crates/agent-gateway/tests/`:
   - `device_token_e2e.rs` — issue → register capability → list → revoke.
   - `shell_control_ws.rs` — wiremock-backed shell connects, sends `Heartbeat`, receives `Replay` request.

> **Why no `DeviceHub` abstraction?** The v1.1 review proposed wrapping the
> `device_tokens` table + WS route + heartbeat in a `DeviceHub` trait.
> Rejected: there is exactly one device class today (Tauri shell). The four
> SQL columns + three handlers + one WS route are simpler than the
> indirection. Add the abstraction when a second device class lands
> (e.g. headless CI runners) and the existing code can be lifted unchanged
> into the first impl.

**Gate.** `cargo test -p agent-gateway` green; new endpoints visible in `/swagger-ui`; feature flag off → endpoints return 404.

---

### Phase 6 — Docker, CI/CD, packaging — **2 units**

**Steps**
1. **`docker-compose.yml`**: add `web` service (Node 22 image, `node build`, profile `full`). **Do not** containerise the Tauri shell.
2. **GitHub Actions — `ci.yml` (PR)**:
   - Cache `~/.pnpm-store`, `target/`, `.turbo/`.
   - Steps: `pnpm i --frozen-lockfile`, `pnpm -w lint`, `pnpm -w test`, `pnpm -w build`, `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `just types && git diff --exit-code packages/types/`.
3. **GitHub Actions — `release-shell.yml` (tag `shell-v*`)**:
   - Matrix:
     - `macos-14` → universal `.dmg` (signed + notarized)
     - `windows-2022` → `.msi` (Azure Trusted Signing)
     - `ubuntu-24.04` → `.AppImage` + `.deb`
     - `macos-14` (iOS) → `.ipa` (Apple cert + provisioning profile)
     - `ubuntu-24.04` (Android) → signed `.aab` + `.apk`
   - Use `tauri-action@v0`.
   - Upload all artifacts to GitHub Releases.
   - Auto-update channel: `tauri-plugin-updater` minisign keys (private in GH OIDC vault).
4. **SBOM + provenance** per artifact: `cargo cyclonedx` + `cyclonedx-npm`. SLSA L3 provenance via `slsa-framework/slsa-github-generator`.
5. **Renovate-bot** config (`renovate.json`): weekly Tauri + Rig + SvelteKit minor bumps; pin patch versions on Tauri plugins (high churn risk).

**Gate.** Tagged release dry-run produces signed installers/IPAs/AABs for **all five matrix entries**.

---

### Phase 7 — Verification & ship — **1 unit**

**Steps**
1. **Automated**:
   - Playwright e2e on `apps/web`: login → workspace CRUD → file upload → capability search.
   - `wdio` + Tauri driver on shell: launch → open tab → record 3 steps → save → reload → list trace.
   - Per-platform smoke matrix in CI nightly:
     - macOS: AppleScript-driven launch + screenshot
     - Windows: `WinAppDriver` + recording assertion
     - iOS Simulator: `xcrun simctl` boot + Detox
     - Android Emulator: `adb shell am start` + `uiautomator` assertion
2. **Manual** via `plan-browser-verifier` skill: capture screenshots + console + network for `/dashboard`, `/capabilities`, shell home, recorder modal, replay dry-run output. **One screenshot per platform** in the release notes.
3. **Security review**:
   - CSP blocks third-party JS (insert `<script src="https://evil.example">` into a recorded page → confirm blocked).
   - Tauri capabilities allowlist: `tauri inspect` shows zero wildcard scopes.
   - `cargo audit`, `pnpm audit --prod`, `osv-scanner`, `trivy` on each container.
   - Privacy manifest validates on iOS (App Store Connect API check).
4. **Docs**:
   - Update [docs/arch.md](arch.md) §3 with new services, §2 with new layout.
   - Add `docs/browser-shell-user-guide.md` (Quick start, recording, replay, troubleshooting per platform).
   - Add `docs/ops/signing.md` (cert rotation per platform; CI fails 30 days before expiry).
5. Tag `v0.4.0`; ship.

---

## 3. Cross-cutting non-negotiables

- **One `src-tauri/` for all platforms.** Mac/Win/iOS/Android share recorder, tabs, Stronghold, OTel, CSP. Platform-specific code lives behind `#[cfg(target_os = "...")]` only when unavoidable.
- **No new top-level Cargo workspaces.** `apps/browser-shell/src-tauri` joins the existing one (Phase 1 step 6).
- **No global state in Tauri.** Use `tauri::State<Arc<Mutex<...>>>`; mirror `agent_gateway::state::AppState`.
- **No secrets in repo.** Tauri Stronghold for device tokens; GH OIDC vault for signing certs.
- **No bypassing `CapabilityProvider`.** The shell is a normal `remote_mcp` client; the backend already knows how to talk to it.
- **A11y from day one.** Every `packages/ui` component passes `axe-core` zero serious/critical; visible focus rings; full keyboard nav; reduced-motion respected; iOS/Android hit targets ≥ 44 pt.
- **Telemetry parity.** Browser + shell emit OTel spans with `tenant.id`, `session.id`, `capability.name`, **plus `platform` ∈ {web, macos, windows, ios, android}** and **`shell.kind` ∈ {browser, headless, mobile}** (generic for future shells). End-to-end stitched in Jaeger.
- **Feature flag.** All shell routes + replay capability behind `CONUSAI_FEATURE_BROWSER_SHELL=1` until Phase 7.
- **PII redaction is non-bypassable.** Recorder hard-codes the filter list; users cannot override it via config or command-line.

---

## 4. Risks & mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Tauri 2 plugin churn | M | Pin exact patch versions; renovate-bot weekly; CI matrix on Tauri minor releases. |
| WebView CSP breaks recorded sites | M | Per-tab CSP override only for the injected `data-testid` script; leave site CSP intact otherwise. |
| Recorder PII leakage | **H** | Hard-coded filter list; password-region screenshot blur; opt-in upload; user consent toggle (iOS App Store requirement). |
| Replay non-determinism | M | Phase 1 ships **dry-run only**; live replay deferred to v0.4.1 after real traces are collected. |
| Two UIs (Askama + SvelteKit) drift | L | Freeze Askama at current scope; all new UI to `apps/web` + `packages/ui`. Remove Askama in v0.5. |
| Code-signing cert expiry | M | Document rotation per platform in `docs/ops/signing.md`; CI fails 30 days before expiry. |
| iOS App Store rejection (recorder) | **H** | Privacy manifest + explicit consent toggle + dry-run only; submit a **TestFlight** build first; document Apple review notes in `docs/ops/ios-review.md`. |
| Android WebView fragmentation | M | `minSdkVersion = 26` (Android 8+); auto-updating system WebView ≥ 100 enforced at launch with friendly upgrade prompt. |
| Windows SmartScreen warning | M | Azure Trusted Signing builds reputation faster than EV USB; ship MSI + portable exe; document the first-week SmartScreen warmup. |
| Phase 0.5 rename breaks downstream | L | Backward-compat re-exports for one release; `#[deprecated]` warnings; full eval suite as gate. |

---

## 5. Effort summary

| Phase | Weight | Depends on | Host required |
|---|---|---|---|
| 0   Branch + ADRs | 1 | — | any |
| 0.5 Capability rename | 1–2 | 0 | any |
| 1   Monorepo skeleton | 1–2 | 0.5 | any |
| 2   Shared packages | 2–3 | 1 | any |
| 3   `apps/web` refactor | 2 | 2 | any |
| 4   Browser-shell scaffold + desktop | 5–7 | 2 | any (mac for signed `.dmg`) |
| 4M  macOS polish | 0–1 | 4 | macOS |
| 4W  Windows MSI | 1–2 | 4 | Windows or cross |
| 4i  iOS | 2–3 | 4, 4M | macOS |
| 4A  Android | 2–3 | 4 | any |
| 5   Backend wiring | 2 | 4.7 | any |
| 6   Docker + CI matrix | 2 | 3, 4M, 4W, 4i, 4A | any |
| 7   Verify + ship | 1 | all | any |
| **Total** | **22–30 weight** | | |

**Strict ordering:**
- **Phase 0.5 must merge before any new code below.**
- Do not start Phase 4 before Phase 2 is green.
- Do not start Phase 5 before Phase 4.7 (registration) is green.
- Phase 4i requires Phase 4M. Phase 4W and Phase 4A are parallelisable with 4i.
- Phase 6 depends on **all** Phase-4 sub-targets being green.

---

## 6. Open questions

1. **Apple Developer Team ID** — needed before Phase 4i CI can sign. Owner?
2. **Azure Trusted Signing tenant** — needed before Phase 4W CI. Owner?
3. **Google Play developer account + upload key** — needed before Phase 4A AAB upload.
4. **Replay capability scope** — confirm v0.4.0 ships dry-run only (no live browser-driving replay). Plan assumes yes per task §4.8 + §4 risks.
5. **Auto-update channels per platform** — single `stable` channel, or per-platform (`stable-mac`, `stable-win`, …)? Default assumption: single channel.

Resolve these before starting Phase 4M / 4W / 4i / 4A respectively.

---

## 7. Definition of done (v0.4.0)

1. All 8 phases green; CI matrix produces signed installers for **macOS (.dmg), Windows (.msi), iOS (.ipa), Android (.aab)** plus the web bundle.
2. Recorder works on every target; saved trace appears in `/v1/workspaces` tree; `TraceReplayCapability` produces a dry-run plan when invoked from the agent loop.
3. `cargo clippy --workspace -- -D warnings` and `pnpm -w lint` clean.
4. `axe-core` zero serious/critical violations across `packages/ui` Histoire stories.
5. Telemetry: a single trace in Jaeger spans browser → gateway → capability → workspace materialisation, with `platform` + `shell.kind` attributes set correctly per source.
6. `docs/browser-shell-user-guide.md` published; `docs/ops/signing.md` lists rotation procedure for every cert/keystore.
7. Privacy manifest validated; TestFlight build approved by Apple review.
8. Feature flag `CONUSAI_FEATURE_BROWSER_SHELL=1` flipped to default-on **only** at v0.4.0 tag.
9. `SessionTrace`, `UserStep`, `SessionRecorder`, and `TraceSource` live in `crates/common` / `agent-core`; adding a new shell or trace producer requires implementing `SessionRecorder` (client side) **only** — no changes to the gateway, registry, or replay capability.

---

## 8. Reviewer feedback explicitly rejected (with rationale)

The v1.1 review proposed three further abstractions; this plan rejects them
as premature per implementation discipline ("only changes directly requested
or clearly necessary"). Each is revisitable when its real second consumer
appears.

| Proposal | Why rejected today | When to revisit |
|---|---|---|
| Extract `recorder`, `tabs`, `telemetry` into 3 separate `tauri-plugin-conusai-*` crates under `packages/tauri-plugins/` | One shell consumer today. Plugin extraction means: separate `Cargo.toml`s, plugin-build pipeline, version pinning across the workspace, and ≈ 4–6 AI-h with zero behavioural payoff. The trait boundaries already in §4.4/§4.5/§4.9 (`SessionRecorder`, tab Tauri commands, the OTel bridge module) make a future extraction mechanical. | When a second Tauri app (headless CI runner, separate desktop tool) needs the same primitives. |
| `DeviceHub` trait wrapping `device_tokens` + WS route + heartbeat | One device class today. Four SQL columns + three handlers + one WS route is fewer LOC than the trait + impl + factory. | When a second device class lands (CI runner, mobile-only client). |
| Dynamic capability discovery on shell launch (shell queries its own cards and POSTs them) | Shell exposes exactly **one** capability (`trace.replay`). Static JSON is 12 lines and trivially auditable. | When the shell exposes ≥3 capabilities or capability set varies per install. |

These rejections shave ~5 AI-h off v1.1's proposed 26–34 budget while
leaving every door open via the accepted trait seams.
