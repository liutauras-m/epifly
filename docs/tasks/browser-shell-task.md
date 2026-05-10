# ConusAI Browser Shell — Implementation Plan

> **Scope.** Add a Tauri 2 desktop "Browser Shell" app and a shared SvelteKit frontend (`apps/web`) to the existing Cargo monorepo, reusing the v0.3.2 backend (`CapabilityProvider`, `CapabilityCard`, `ArtifactBridge`, `SemanticCapabilityRouter`, `/v1/files`, `/v1/workspaces`, `/admin/capabilities/register`, `/v1/capabilities/search`).
>
> **Source intent.** Derived from `docs/tasks/browser-shell.md` v0.3.4, reconciled against the actual workspace state (May 2026). Where the source plan referenced symbols/endpoints that already exist or need a different name, this plan uses the canonical ones.
>
> **Best-practice baseline (2026).** Tauri 2.x · SvelteKit 2 + Svelte 5 runes · pnpm 9 + Turborepo 2 · Vite 6 · TypeScript 5.7 · Biome (lint+format) · Vitest + Playwright · GitHub Actions matrix builds · `wry` WebView2/WKWebView with per-tab partitions · OpenTelemetry browser SDK exporting to the existing OTLP collector.

---

## 0. Reconciliation with current code

What already exists and **must be reused** (do not reinvent):

| Concern | Existing symbol / route | Path |
|---|---|---|
| Capability trait | `CapabilityProvider` | [apps/backend/crates/agent-core/src/tools/provider.rs](apps/backend/crates/agent-core/src/tools/provider.rs) |
| Card / metadata | `CapabilityCard` | [apps/backend/crates/agent-core/src/tools/card.rs](apps/backend/crates/agent-core/src/tools/card.rs) |
| Registry (to be renamed → `CapabilityRegistry` in Phase 0.5) | `ToolRegistry` | [apps/backend/crates/agent-core/src/tools/registry.rs](apps/backend/crates/agent-core/src/tools/registry.rs) |
| Self-registration endpoint | `POST /admin/capabilities/register` (`remote_mcp` kind) | [apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs](apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs#L551) |
| Semantic capability search | `GET /v1/capabilities/search?q=…` | [apps/backend/crates/agent-gateway/src/routes/search.rs](apps/backend/crates/agent-gateway/src/routes/search.rs) |
| Artifact materialisation | `ArtifactBridge::process_if_artifacts` (auto-invoked after every tool call) | [apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs](apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs) |
| File upload | `POST /v1/files` (multipart, returns download token) | [apps/backend/crates/agent-gateway/src/routes/files.rs](apps/backend/crates/agent-gateway/src/routes/files.rs) |
| Workspace node CRUD | `POST /v1/workspaces` (`kind: file`/`folder`) | [apps/backend/crates/agent-gateway/src/routes/workspaces.rs](apps/backend/crates/agent-gateway/src/routes/workspaces.rs) |
| Realtime stream | `GET /api/realtime/workspace` (WebSocket) | [apps/backend/crates/agent-gateway/src/routes/realtime.rs](apps/backend/crates/agent-gateway/src/routes/realtime.rs) |
| Existing UI | Askama (`templates/`, `assets/`) — kept until web app is at parity | [apps/backend/crates/agent-gateway/templates](apps/backend/crates/agent-gateway/templates) |

What is **missing** and must be added:
1. `apps/web` (SvelteKit) and `apps/browser-shell` (Tauri 2 + SvelteKit static).
2. `packages/ui`, `packages/types`, `packages/sdk` shared packages.
3. Root `pnpm-workspace.yaml`, `turbo.json`, `package.json`, `justfile`.
4. A `BrowserShellReplayCapability` (kind = `remote_mcp`) — lives in `agent-core/src/capabilities/`, **not** in the gateway — that takes a `SessionTrace` artifact and produces a replay plan via Rig.
5. CSP + capability-allowlist hardening on the Tauri side.
6. CI matrix that builds Tauri for macOS (universal), Windows MSI, Linux AppImage.
7. **Canonical-name alignment.** Rename `agent-core/src/tools/` → `agent-core/src/capabilities/` and `ToolRegistry` → `CapabilityRegistry` to match the rest of the v0.3 vocabulary (`CapabilityProvider`, `CapabilityCard`, `CapabilityAdmin`, `CapabilitySpecFactory`). Mechanical, single-commit, no behaviour change.

The source plan’s `frontendDist` ("../dist") and `BulkCapabilityFactory` mention need no change — both already exist in code.

---

## 1. Target monorepo layout

```
conusai-platform/
├── apps/
│   ├── backend/                  # existing Rust gateway + capabilities (unchanged shape)
│   ├── web/                      # NEW SvelteKit web app (SSR via adapter-node)
│   └── browser-shell/            # NEW Tauri 2 shell
│       ├── src/                  #   SvelteKit (adapter-static) — reuses packages/ui
│       └── src-tauri/            #   Rust crate (added to root Cargo workspace)
├── packages/
│   ├── ui/                       # NEW shared Svelte 5 components
│   ├── types/                    # NEW shared TS types (mirror common::artifact / WorkspaceNode)
│   └── sdk/                      # NEW @conusai/sdk — typed client for /v1 + /admin
├── services/                     # unchanged
├── docker/                       # unchanged
├── docs/                         # unchanged
├── Cargo.toml                    # extend [workspace.members]
├── package.json                  # NEW (root, private)
├── pnpm-workspace.yaml           # NEW
├── turbo.json                    # NEW
└── biome.json                    # NEW
```

---

## 2. Phased plan

Each phase ends with a hard gate: `cargo check && cargo clippy -- -D warnings && pnpm -w build && pnpm -w test`. Do not start the next phase until the gate passes. Use the **plan-browser-verifier** skill at the end of every phase that ships UI to drive a real browser/Tauri build and capture screenshots.

### Phase 0 — Branch, prerequisites, ADRs (1 unit)

1. `git checkout -b feat/browser-shell`.
2. Add two ADRs:
   - `docs/adr/0005-tauri-browser-shell.md` — Tauri 2 vs Electron, SvelteKit vs Next, SSR vs static, why we keep Askama UI in parallel during transition. Record that **SvelteKit becomes the canonical v0.4 frontend** (supersedes the tentative Next.js note in `docs/arch.md` § 0.2).
   - `docs/adr/0006-capability-module-rename.md` — capture the `tools/` → `capabilities/` and `ToolRegistry` → `CapabilityRegistry` rename rationale.
3. Tooling pins (commit a `.tool-versions` and document in `README.md`):
   - Node 22 LTS, pnpm 9, Rust 1.95 (already in `rust-toolchain.toml`), `just` 1.36.
4. Install once locally (no global installs in CI — use `corepack`):
   ```bash
   corepack enable && corepack prepare pnpm@9 --activate
   cargo install tauri-cli --version "^2" --locked
   cargo install just --locked
   ```

### Phase 0.5 — Canonical capability rename (1–2 units, single commit)

Mechanical, behaviour-preserving rename. **Must merge before any new code below.**

1. `git mv apps/backend/crates/agent-core/src/tools apps/backend/crates/agent-core/src/capabilities`.
2. `git mv apps/backend/crates/agent-core/src/capabilities/registry.rs apps/backend/crates/agent-core/src/capabilities/capability_registry.rs`.
3. Use the language-server rename to change the symbol `ToolRegistry` → `CapabilityRegistry` (updates `mod.rs` re-exports, `agent-gateway::state::AppState`, `RegisteredToolAdmin`/`CapabilityAdmin` callers, all `crates/jobs` and `evals` references).
4. Keep public type aliases for one release: `pub use capability_registry::CapabilityRegistry as ToolRegistry;` in `agent-core::lib` to avoid breaking external in-flight branches. Mark `#[deprecated]`.
5. Run `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `make verify`.
6. Update `docs/arch.md` symbol table and `/memories/repo/dynamic-tool-registration.md`.

**Gate.** Green build + zero behavioural diff (run existing eval suite).

### Phase 1 — Frontend monorepo skeleton (1–2 units)

1. Root `package.json` with `"packageManager": "pnpm@9.15.0"`, `"private": true`, `"engines.node": ">=22"`.
2. `pnpm-workspace.yaml`:
   ```yaml
   packages:
     - "apps/web"
     - "apps/browser-shell"
     - "packages/*"
   ```
3. `turbo.json` with pipelines: `build`, `dev` (persistent), `lint`, `test`, `check-types`. Cache `.svelte-kit/`, `dist/`, `build/`.
4. Add `biome.json` (replace ESLint+Prettier — 2026 default). Configure import-sorting + Svelte plugin.
5. `.gitignore` additions: `node_modules/`, `.turbo/`, `.svelte-kit/`, `apps/*/build`, `apps/*/dist`, `apps/browser-shell/src-tauri/target/`.
6. Wire `Cargo.toml` workspace to include `apps/browser-shell/src-tauri` (add to `members`).

**Gate.** `pnpm install` succeeds and `cargo metadata` still resolves.

### Phase 2 — Shared packages (2 units)

#### `packages/types`
- Generate from backend OpenAPI via a `just types` recipe that boots a transient gateway instance (sqlite-in-memory feature or docker-compose `infra` profile) and runs `pnpm openapi-typescript http://localhost:8080/api-docs/openapi.json -o src/openapi.d.ts`. The recipe runs in CI on every PR so drift fails the build.
- **Valibot post-processor.** A second step (`scripts/openapi-to-valibot.ts`) emits matching [Valibot](https://valibot.dev) schemas to `src/valibot.ts` (Valibot is the 2026 lightweight successor to Zod — ~1 kB tree-shaken, native Svelte/Tauri fit). The SDK uses these for runtime request/response validation, giving zero-drift end-to-end safety.
- Wrap with hand-written domain types (`SessionTrace`, `UserStep`, `CapabilityCard`, `WorkspaceNode`, `ControlMessage`) that match `common::artifact::Artifact` / `common::memory::workspace::WorkspaceNode`.
- Export types **and** Valibot schemas; no other runtime code.

#### `packages/sdk` (`@conusai/sdk`)
- Thin typed client built on native `fetch`, using `packages/types`.
- Methods (one per endpoint actually used):
  - `auth.login`, `capabilities.list`, `capabilities.search(q, limit)`, `capabilities.register(manifest)`, `files.upload(file)`, `workspaces.create(node)`, `realtime.subscribe()` (WebSocket wrapper with auto-reconnect + jittered backoff).
- `BrowserShellControlClient` — typed wrapper around the new `/v1/shells/{device_id}/control` WebSocket (auto-reconnect, jittered backoff, structured `ControlMessage` enum mirroring the Rust side).
- Auth: pluggable `tokenProvider: () => Promise<string>`; never store tokens in localStorage — use SvelteKit `cookies` (web) and Tauri `Stronghold` plugin (shell).
- Exponential-backoff retry on 5xx + idempotent verbs only.
- 100% test coverage with `vitest` + `msw` mocked HTTP.

#### `packages/ui`
- Svelte 5 (runes) component library, Tailwind 4 (oxide engine) preconfigured to match `docs/ui-design.md` (warm neutrals, single teal accent, Archivo/Inter/Space Mono — see **frontend-design** skill).
- Components needed for the shell + web parity: `AppShell`, `CommandPalette`, `CapabilityCard`, `WorkspaceTree`, `RecorderControls`, `TabStrip`, `ToastHost`, `ArtifactPreview`.
- All components import from `@conusai/types`, never from app code.
- Storybook 8 (or Histoire — lighter for Svelte) with one story per component.

**Gate.** `pnpm -w build`, `pnpm -w test`, axe-core a11y check on every story.

### Phase 3 — `apps/web` (SvelteKit) (3 units)

1. Scaffold: `pnpm create svelte@latest web --template skeleton --types typescript`.
2. `adapter-node` (we deploy behind the existing Rust gateway via reverse proxy; SSR for auth + SEO; falls back to `adapter-static` if we later choose pure CDN).
3. Routes (parity with current Askama UI, then expansion):
   - `/login` (POSTs to `/v1/auth/login`, sets HttpOnly cookie via SvelteKit `+server.ts`).
   - `/dashboard` — workspace tree + chat panel.
   - `/capabilities` — list, search, super-admin registration form.
   - `/workspaces/[id]` — node view, artifact preview.
4. Use `@conusai/sdk` everywhere; no inline `fetch`. Server-side calls go through `event.fetch` so cookies propagate.
5. CSP via SvelteKit `handle` hook: `default-src 'self'; connect-src 'self' https://api.conusai.com wss://api.conusai.com; img-src 'self' data: blob:`.
6. Auth: short-lived JWT in HttpOnly+Secure+SameSite=Lax cookie; CSRF token via SvelteKit form actions.
7. Telemetry: `@vercel/otel` browser SDK → existing OTel collector at `:4318`.

**Gate.** Lighthouse ≥ 95 on `/dashboard`. Playwright e2e: login → create workspace → upload file → see file in tree.

### Phase 4 — `apps/browser-shell` (Tauri 2) (5–7 units)

This is the meaty phase. Build incrementally; ship a working slice each sub-step.

**4.1 Scaffold.**
```bash
cd apps && pnpm create tauri-app@latest browser-shell \
  --template svelte-ts --manager pnpm
```
Configure SvelteKit with `adapter-static` (`fallback: 'index.html'`) so output is bundleable into `src-tauri`. Wire shared deps: `pnpm add @conusai/ui @conusai/sdk @conusai/types`.

**4.2 Tauri config (`src-tauri/tauri.conf.json`).**
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ConusAI Browser",
  "identifier": "ai.conusai.browser",
  "version": "0.1.0",
  "build": {
    "frontendDist": "../build",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm --filter browser-shell dev",
    "beforeBuildCommand": "pnpm --filter browser-shell build"
  },
  "app": {
    "security": {
      "csp": "default-src 'self'; connect-src 'self' http://localhost:8080 https://api.conusai.com wss://api.conusai.com; img-src 'self' data: https: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'",
      "capabilities": ["main-capability"]
    }
  }
}
```

**4.3 Capabilities allowlist (`src-tauri/capabilities/main-capability.json`).** Tauri 2 uses the new permissions system — list each plugin permission explicitly:
- `core:default`, `core:webview:allow-create-webview-window`, `core:event:default`,
- `webview:allow-internal-toggle-devtools` (dev only), `dialog:default`, `fs:scope-app-data`,
- `http:default` (scoped to `https://api.conusai.com` and dev `http://localhost:8080`),
- `stronghold:default` (token vault).

No wildcard scopes. No `dangerousRemoteDomainIpcAccess`.

**4.4 Multi-tab WebView (`src-tauri/src/tabs.rs`).** One `WebviewWindow` per tab, each with a unique `data_directory` (per-tab partition for cookies/storage isolation). Tauri commands: `create_tab(url) -> tab_id`, `close_tab(tab_id)`, `navigate(tab_id, url)`, `list_tabs()`.

**4.5 Step recorder (`src-tauri/src/recorder.rs`).**
- A `Recorder` struct (one per tab) collects `UserStep { ts, kind, selector, value, url, screenshot? }`.
- Inject a JS bridge script into every webview via `WebviewBuilder::initialization_script` that listens for `click`, `input`, `submit`, `navigation` and calls `__TAURI__.invoke('record_step', step)`.
- Selectors: prefer `data-testid` → unique CSS path → XPath fallback. Never record `password`/`autocomplete=cc-*` fields (PII filter list).
- Screenshots (optional, throttled): `webview.take_screenshot()` PNG → base64.
- Output: `SessionTrace { id, started_at, ended_at, steps: Vec<UserStep>, urls: Vec<String> }`.

**4.6 ArtifactBridge integration.** When a recording stops:
1. Serialize `SessionTrace` to JSON.
2. POST to `/v1/files` (multipart) → get download token.
3. POST to `/v1/workspaces` with `kind: "file"`, `name: "session-<ulid>.trace.json"`, metadata `{ source: "browser-shell", trace_id, urls }`.
4. The backend `WorkspaceIndexer` (already running) picks it up; no new endpoint needed.

**4.7 Auto-registration on launch (`src-tauri/src/main.rs`).** On first run, POST `/admin/capabilities/register` with a manifest:
```json
{
  "capability_id": "browser-shell.replay",
  "kind": "remote_mcp",
  "endpoint": "local://tauri",
  "manifest": { "name": "browser_shell_replay", "version": "0.1.0",
    "description": "Replay a recorded browser session against a target URL.",
    "kind": "remote_mcp", "namespace": "browser.shell",
    "tags": ["browser","replay","automation"], "tools": [...] }
}
```
Use a per-install device token (stored in Stronghold) that maps to a service account on the gateway. **Do not use a super-admin JWT.**

**4.8 `BrowserShellReplayCapability` (Rust, in `agent-core/src/capabilities/browser_shell_replay.rs`).**
- Lives in **`agent-core`**, not the gateway — keeps the gateway thin (HTTP/WS only) and makes the capability reusable by future shells (mobile, headless CI).
- Implements `CapabilityProvider` (`kind = remote_mcp`). Naming follows the existing `PromptChainCapability` pattern.
- **Composes `PromptChainCapability`** — the new struct holds an inner `PromptChainCapability` (built via `AgentBuilder` + a `CompletionProvider`) and delegates LLM orchestration to it. **No bespoke Rig glue code.** This is SRP: replay = trace-loading + prompt construction + post-processing, all of which wrap the proven chain primitive.
- Input schema: `{ trace_node_id: ULID, target_url?: string, dry_run?: bool }`.
- Loads the trace from `workspace_nodes`, hands the serialised `SessionTrace` to the inner `PromptChainCapability` (system prompt: “Translate these steps into a deterministic replay plan…”), and lets `ArtifactBridge::process_if_artifacts` materialise the result. For Phase-1 we ship **dry-run only** (returns the plan as a `ToolOutput.artifacts[0]`).

**4.9 Shared UI + telemetry on the shell.** Reuse `AppShell`, `TabStrip`, `RecorderControls`, `WorkspaceTree`, `CommandPalette` from `packages/ui`. Use Tauri's `event` plugin for tab/recorder events. Add `tauri-plugin-opentelemetry` (or, if upstream is too immature, a ~50-line OTLP/HTTP bridge in `src-tauri/src/telemetry.rs`) so Rust-side spans flow to the same OTel collector at `:4318` with identical `tenant.id` / `session.id` / `capability.name` attributes — traces stitch end-to-end with the gateway in Jaeger.

**Gate.** `cargo tauri build` produces a signed macOS `.app` (dev cert) and a Linux AppImage. Manual smoke: open Swedbank login page (do not enter creds), click record → save → trace appears in `/v1/workspaces` tree.

### Phase 5 — Backend wiring (2 units)

1. Add `agent-core/src/capabilities/browser_shell_replay.rs` (see 4.8). Register via `BuiltinFactory` (or a dedicated `BrowserShellFactory`) in `CapabilityRegistry::with_default_factories`.
2. Gateway stays thin: extend `CapabilityRegisterRequest` to accept a `device_token` (replaces super-admin JWT for shells). Add a `device_tokens` table + `/admin/devices` issuance endpoint. Use blake3 hash for storage.
3. New WebSocket route `GET /v1/shells/{device_id}/control` (Bearer device token) — pure transport in the gateway, dispatches to `BrowserShellReplayCapability` via an `Arc<ShellControlHub>` injected through `AppState`.
4. Quotas: extend `RouterQuotaConfig` to count replay invocations (`CONUSAI_MAX_REPLAYS_PER_TURN`, default 3).
5. OpenAPI: annotate new routes; regenerate `packages/types/src/openapi.d.ts` in CI via `just types`.

**Gate.** `cargo test -p agent-gateway` passes; new endpoints visible in `/swagger-ui`.

### Phase 6 — Docker, CI/CD, packaging (1–2 units)

1. **`docker-compose.yml`.** Add `web` service (Node 22 image running `node build`, profile `full`). Do **not** containerise the Tauri shell.
2. **`justfile`** at repo root (2026 community standard for mixed Rust + TS monorepos). Existing `Makefile` is kept for backend-only legacy targets (`make db-up`, `make verify`) and will be deprecated in v0.5.
   ```just
   web-dev:        pnpm --filter web dev
   shell-dev:      pnpm --filter browser-shell tauri dev --no-watch
   shell-build:    pnpm --filter browser-shell tauri build
   types:          ./scripts/gen-types.sh
   verify:         cargo clippy --workspace -- -D warnings && cargo test --workspace && pnpm -w test
   ```
   `--no-watch` matches the snappier `web-dev` iteration loop — the SvelteKit Vite dev server already supplies HMR, so disabling Tauri’s file watcher avoids double rebuilds.
3. **GitHub Actions.**
   - `ci.yml` (PR): `pnpm i --frozen-lockfile`, `pnpm -w lint`, `pnpm -w test`, `pnpm -w build`, `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`.
   - `release-shell.yml` (tag `shell-v*`): matrix on `macos-14` (universal via `--target universal-apple-darwin`), `windows-2022` (MSI + signed via Azure Trusted Signing), `ubuntu-24.04` (AppImage + .deb). Use `tauri-action@v0`. Upload to GitHub Releases. Sign updates with `tauri-plugin-updater` minisign keys (store private key in GH OIDC-fronted vault, never raw secrets).
4. **SBOM + provenance.** `cargo cyclonedx` + `cyclonedx-npm` per artifact, attach to release. SLSA L3 provenance via `slsa-framework/slsa-github-generator`.

**Gate.** Tagged release dry-run produces signed installers for all three OSes.

### Phase 7 — Verification & ship (1 unit)

1. **Automated.**
   - Playwright test: web app full flow (login → create workspace → upload → search capability).
   - `wdio` + Tauri driver: shell launches, opens a tab, records 3 steps, saves trace, reloads and lists trace from `/v1/workspaces`.
2. **Manual.** Use **plan-browser-verifier** skill: capture screenshots + console + network for `/dashboard`, `/capabilities`, shell home, recorder modal, replay dry-run output.
3. **Security review.**
   - Confirm CSP blocks third-party JS in shell (test by inserting `<script src="https://evil.example">` into a recorded page).
   - Confirm Stronghold-stored device token never leaves the process.
   - Run `cargo audit`, `pnpm audit --prod`, `osv-scanner`.
4. **Docs.**
   - Update `docs/arch.md` § 3 with new services and § 2 with new layout.
   - Add `docs/browser-shell-user-guide.md` (Quick start, recording, replay, troubleshooting).
5. Tag `v0.4.0`, write release notes, ship.

---

## 3. Cross-cutting non-negotiables

- **No new top-level Cargo workspaces.** `apps/browser-shell/src-tauri` joins the existing one.
- **No global state in Tauri.** Use `tauri::State<Arc<Mutex<...>>>`; reuse the patterns from `agent_gateway::state::AppState`.
- **No secrets in repo or env files committed.** Use `.env.local` (already gitignored) and Tauri Stronghold.
- **No bypassing the existing CapabilityProvider abstraction.** The shell is a normal `remote_mcp` client; the backend already knows how to talk to it.
- **A11y from day one.** All `packages/ui` components must pass `axe-core` with zero serious/critical violations; visible focus rings; full keyboard navigation; reduced-motion respected.
- **Telemetry parity.** Browser + shell emit OTel spans with the same `tenant.id`, `session.id`, `capability.name` attributes as the Rust gateway, so traces stitch end-to-end in Jaeger.
- **Feature flags.** Gate the shell-only routes (`/v1/shells/*`, `ReplayCapability`) behind `CONUSAI_FEATURE_BROWSER_SHELL=1` until Phase 7 ships.

---

## 4. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Tauri 2 plugin churn | Pin exact patch versions; renovate-bot weekly. |
| WebView CSP breaking recorded sites | Per-tab CSP override only for `data-testid` injection script; leave site CSP intact otherwise. |
| Step-recorder PII leakage | Selector + value redaction list, screenshots blurred for `<input type=password>` regions, opt-in upload. |
| Replay non-determinism | Phase 1 ships **dry-run only**; live replay deferred to v0.4.1 after we collect real traces. |
| Two UIs (Askama + SvelteKit) drift | Freeze Askama feature set at current scope; all new UI work goes to `apps/web`. Remove Askama in v0.5. |
| Code-signing certs expire | Document rotation in `docs/ops/signing.md`; CI fails 30 days before expiry. |

---

## 5. Effort summary

Units are relative weights, not time estimates. The recorder + tab-isolation work in Phase 4 is by far the heaviest.

| Phase | Weight | Dependencies |
|---|---|---|
| 0 Branch + ADRs | 1 | — |
| 0.5 Capability rename (single commit) | 1–2 | 0 |
| 1 Monorepo skeleton | 1–2 | 0.5 |
| 2 Shared packages | 2–3 | 1 |
| 3 `apps/web` | 3 | 2 |
| 4 `apps/browser-shell` | 7–9 | 2 |
| 5 Backend wiring | 2 | 4.7 |
| 6 Docker + CI + justfile | 1–2 | 3, 4 |
| 7 Verify + ship | 1 | all |
| **Total** | **19–25 weight** | |

Execute strictly in order; **Phase 0.5 must merge before any new code below**, do not start Phase 4 before Phase 2 is green, and do not start Phase 5 before Phase 4.7 (registration) is green.

**ConusAI Browser Shell — iOS Target Support (v0.4.1 Extension Plan)**

The query “How to run as iOS app!” is answered with the **canonical 2026 Tauri 2 approach**: extend the existing `apps/browser-shell` (already scaffolded with `pnpm create tauri-app@latest --template svelte-ts`) to a **single-codebase multi-platform app** (desktop + iOS + future Android). No new crates, no fork, no extra frontend. The same SvelteKit 5 (adapter-static) build, same `CapabilityProvider`/`BrowserShellReplayCapability`, same `SessionTrace` recorder (JS injection works identically on WKWebView), and same `remote_mcp` registration flow are reused 100 %. Tauri 2’s mobile support is mature, production-ready, and the idiomatic choice (Wry → WKWebView, new permission model, per-app Stronghold, unified `tauri.conf.json`).

**Why this is the best approach (2026 standards)**  
- Single `src-tauri/` for all targets → SRP preserved, maintenance cost near-zero.  
- `tauri ios init` + `tauri ios dev/build` are the official, community-canonical CLI surface (v2.tauri.app).  
- No custom Xcode scripting; Tauri CLI orchestrates `xcodebuild` + Cargo.  
- Frontend hot-reload works via `TAURI_DEV_HOST` env var (SvelteKit Vite auto-adapts).  
- Recorder, tabs, Stronghold, OTel, and CSP all transfer with **zero code changes** (WKWebView supports the same `initialization_script` + `invoke` bridge).  
- iOS replay capability remains a normal `remote_mcp` client — backend unaware of the platform.

**Effort estimate**: 8–12 AI-hours / ~65k tokens (mostly config + CI + testing).  
- 2 hours setup & `tauri ios init`  
- 3 hours config + recorder validation  
- 2 hours justfile + CI matrix  
- 1–5 hours manual verifier + device testing  

This slots cleanly as **Phase 4.10** (immediately after Phase 4.9 telemetry). Do **not** start before Phase 4 desktop gate passes and Phase 0.5 rename is merged.

### Phase 4.10 — iOS Target (execute after Phase 4.9)

1. **Prerequisites (macOS host only — required for Xcode)**  
   - macOS 14+ (Sonoma or later).  
   - Xcode 16+ (install from Mac App Store; open once to accept license).  
   - Command-line tools: `xcode-select --install`.  
   - Rust iOS targets (run once):  
     ```bash
     rustup target add aarch64-apple-ios aarch64-apple-ios-sim
     ```  
   - Update `.tool-versions` and `README.md` with note: “iOS builds require macOS + Xcode 16+”.

2. **Initialize iOS project (single command, inside `apps/browser-shell`)**  
   ```bash
   cd apps/browser-shell
   pnpm tauri ios init
   ```  
   This creates `src-tauri/gen/apple/` (Xcode project) and updates `tauri.conf.json` with the iOS bundle identifier.

3. **Update `src-tauri/tauri.conf.json` (minimal, canonical)**  
   Add/extend the `bundle` and `ios` sections (merge with existing desktop config):
   ```json
   {
     "bundle": {
       "identifier": "ai.conusai.browser",
       "icon": ["icons/128x128.png", "icons/128x128@2x.png", ...],  // reuse existing icons
       "ios": {
         "minimumVersion": "15.0",
         "privacyManifests": true,
         "features": ["camera", "microphone"]  // only if we add them later
       }
     },
     "app": {
       "security": {
         "csp": "default-src 'self'; connect-src 'self' http://localhost:* https://api.conusai.com wss://api.conusai.com; img-src 'self' data: https: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'"
       }
     }
   }
   ```
   **No** changes to `frontendDist`, `beforeBuildCommand`, or capabilities — Tauri reuses the same static build.

4. **Extend `packages/ui` for mobile responsiveness (one-time)**  
   - Add Tailwind mobile-first utilities + viewport meta in `AppShell.svelte` (already planned in design system).  
   - Use Svelte 5 runes + CSS container queries — no new deps.  
   - Test: `pnpm --filter ui storybook` on iPhone 16 simulator viewport.

5. **Update recorder for WKWebView (zero-code change expected)**  
   The existing `src-tauri/src/recorder.rs` + JS bridge works identically (WKWebView supports `initialization_script`, `take_screenshot`, and `invoke`).  
   Add one small guard in `tabs.rs` if needed:
   ```rust
   #[cfg(target_os = "ios")]
   let webview = webview.with_initialization_script(&ios_specific_script()); // optional extra CSP bypass for recorded sites
   ```

6. **New justfile recipes (root `justfile` — canonical mixed-stack standard)**  
   ```just
   shell-ios-dev:    pnpm --filter browser-shell tauri ios dev
   shell-ios-build:  pnpm --filter browser-shell tauri ios build
   shell-ios-sim:    open -a Simulator && pnpm --filter browser-shell tauri ios dev
   ```
   (Keep desktop `shell-dev` untouched.)

7. **CI extension (`release-shell.yml`)**  
   - Add macOS runner job with matrix target `aarch64-apple-ios`.  
   - Use `tauri-action@v0` with `--target aarch64-apple-ios`.  
   - Output: `.ipa` uploaded to GitHub Releases alongside desktop artifacts.  
   - (Note: iOS signing requires Apple Developer account + provisioning profile; store certs in GH OIDC vault.)

8. **Run commands (developer workflow)**  
   **Simulator (recommended for daily dev):**  
   ```bash
   just shell-ios-sim
   ```  
   → Tauri CLI starts Vite dev server on the correct host, launches iOS Simulator, installs & runs the app. Hot-reload works.

   **Physical iOS device:**  
   ```bash
   # First time only
   open -a Xcode
   # Connect device via USB + enable network in Xcode > Window > Devices and Simulators
   just shell-ios-dev --force-ip-prompt
   ```

   **Production build & archive for TestFlight/App Store:**  
   ```bash
   just shell-ios-build -- --export-method app-store
   ```
   IPA lands in `src-tauri/gen/apple/build/arm64/ConusAI_Browser.ipa`.

**Gate (Phase 4.10)**  
- `just shell-ios-dev` launches in Simulator with recorder working (record 3 steps on any site, save trace → appears in workspace tree).  
- `cargo tauri ios build` succeeds (no signing required for simulator).  
- `pnpm -w test && cargo test --workspace` still green.  
- Run **plan-browser-verifier** skill on Simulator screenshots.

**Cross-cutting (unchanged)**  
- Same CSP, Stronghold device token, PII redaction, a11y, feature flag, OTel spans (now with `platform: ios`).  
- Capability registration on launch works unchanged (`remote_mcp`).  
- No new top-level crates or global state.

**Risks & mitigations**  
| Risk | Mitigation |
|------|------------|
| Xcode provisioning | Document in `docs/ops/ios-signing.md`; CI fails early. |
| WKWebView JS bridge latency | Same as desktop; throttled screenshots already in place. |
| iOS App Store review (recorder) | Dry-run only in v0.4.1; add privacy manifest + user consent toggle. |

**Next step**  
After Phase 0.5 rename merges and Phase 4 desktop ships, create PR `feat/browser-shell-ios` with the above changes. I will supply the exact `tauri.conf.json` diff + justfile snippet + CI matrix once you confirm the branch is ready.

This keeps the monorepo clean, fully Rig.rs-aligned, and gives us a true cross-platform Browser Shell with **one codebase**. iOS is now a first-class target — exactly the 2026 standard.

**Phase 4.10 ready to execute after desktop gate.** Let me know when you want the PR diff.

**ConusAI Browser Shell — macOS Native App Run Guide (v0.4.0)**

macOS is the **primary, zero-config desktop target** in the existing `apps/browser-shell` Tauri 2 scaffold (Phase 4.1). No extra `tauri macos init` or new crates are required — the same SvelteKit static build (`adapter-static`), `CapabilityProvider`/`BrowserShellReplayCapability`, `SessionTrace` recorder, per-tab WKWebView isolation, Stronghold, and `remote_mcp` registration all run natively via Wry + Apple’s WKWebView. This is the 2026 canonical Tauri 2 pattern: single `src-tauri/` codebase, universal binary by default, and full reuse of the desktop justfile + pnpm workspace.

**Why this is optimal (Rig.rs-aligned)**  
- macOS builds are the fastest path in the Tauri CLI (`tauri dev` / `tauri build`).  
- Universal binary (arm64 + x86_64) produced automatically.  
- Same CSP, PII redaction, a11y, OTel spans (`platform: macos`), and feature flag as iOS/desktop.  
- No code changes needed — the recorder’s JS bridge and `WebviewWindow` tabs work identically.

**Effort estimate**: 0–1 AI-hour / ~4k tokens (pure documentation + justfile polish; already covered in Phase 4).  
This is **not** a new phase — it is the default outcome of completing Phase 4 (desktop).

### Prerequisites (already satisfied after Phase 0 + Phase 4)
- macOS 14+ (Sonoma or later).  
- Rust 1.95+ with `aarch64-apple-darwin` target (auto-added by `rustup` in Phase 0).  
- `pnpm@9`, Tauri CLI v2 (installed via `cargo install tauri-cli --version "^2" --locked`).  
- Xcode Command Line Tools (only needed for code-signing on release builds): `xcode-select --install`.  
- `just` 1.36 (already pinned).

### Developer Workflow (canonical commands)

**1. Development (hot-reload, recommended daily loop)**  
From the monorepo root:
```bash
just shell-dev
```
(or directly)
```bash
pnpm --filter browser-shell tauri dev
```
- Starts SvelteKit Vite dev server + Tauri dev window.  
- Full HMR for Svelte 5 runes, live recorder testing, and WebSocket control channel.  
- Opens a native macOS window with menu bar and per-tab isolation.

**2. Production Build (signed .app bundle)**  
```bash
just shell-build
```
(or)
```bash
pnpm --filter browser-shell tauri build --target universal-apple-darwin
```
- Output: `apps/browser-shell/src-tauri/target/release/bundle/osx/ConusAI Browser.app` (universal binary).  
- Automatically code-signed with your local developer certificate (or ad-hoc if none configured).  
- Bundle includes all icons, entitlements, and the hardened CSP from `tauri.conf.json`.

**3. Run the built app**  
Double-click the `.app` or:
```bash
open apps/browser-shell/src-tauri/target/release/bundle/osx/ConusAI\ Browser.app
```

**4. Optional: Clean rebuild + signing verification**
```bash
just shell-build -- --clean
codesign -dv --verbose=4 "target/release/bundle/osx/ConusAI Browser.app"
```

### justfile Recipes (already added in Phase 6, macOS-specific)
```just
shell-dev:      pnpm --filter browser-shell tauri dev
shell-build:    pnpm --filter browser-shell tauri build --target universal-apple-darwin
shell-run:      open apps/browser-shell/src-tauri/target/release/bundle/osx/ConusAI\ Browser.app
```

### CI & Release (already in `release-shell.yml`)
- macOS-14 runner produces the universal `.app` + `.dmg` (via `tauri-action@v0`).  
- Uploaded to GitHub Releases on `shell-v*` tags.  
- `tauri-plugin-updater` enabled for seamless in-app updates.

**Gate (already part of Phase 4 desktop gate)**  
- `just shell-dev` launches with recorder working (record 3 steps → trace saved to workspace).  
- `just shell-build` succeeds and the `.app` launches cleanly.  
- `plan-browser-verifier` skill run on the macOS build (screenshots + network trace).

All cross-cutting rules remain enforced: same Stronghold device token, PII redaction, CSP, a11y, telemetry parity (`platform: macos`), and `CONUSAI_FEATURE_BROWSER_SHELL` flag. No global state, no new crates, full SRP.

**Next step**  
After Phase 0.5 rename merges and Phase 4 desktop gate passes, macOS is immediately runnable via `just shell-dev`.  
If you have already completed Phase 4 on your branch, just run the commands above — you’re live on macOS right now.

This is the cleanest, most maintainable macOS path possible. Let me know if you want the exact `tauri.conf.json` snippet for universal signing or the PR diff for any polish.