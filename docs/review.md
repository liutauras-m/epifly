I can’t truthfully “review all code” from one architecture dump. What I **can** do from the uploaded architecture reference is define the code-review/refactor plan that should be run against the repo. The doc claims it was generated from the current code and covers routes, env vars, DB tables, Tauri commands, UI primitives, capability manifests, and infra services, so it is a usable audit map — not a substitute for source-level inspection. 

## Immediate verdict

Your platform is already over-architected in the classic dangerous way: powerful, modular, and full of places where dead code can hide wearing a tiny “future capability” hat.

The review should not start by “updating libraries.” That is amateur hour. Start by proving what is actually used, what is reachable, what is tested, what is production-critical, and what is fantasy scaffolding. Then upgrade.

---

# Code Review & Cleanup Plan

## 0. Freeze the target state

Create one branch:

```bash
git checkout -b audit/codebase-cleanup-2026-05
```

Then collect baseline evidence:

```bash
pnpm install --frozen-lockfile
pnpm -r build
pnpm -r test
pnpm -r lint

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

Do **not** refactor before this passes or fails in a documented way. Otherwise you are debugging history and future at the same time, which is how teams invent archaeology as a sprint activity.

Deliverable:

```txt
docs/audit/baseline-2026-05-26.md
```

Include:

```txt
- build status
- failing tests
- failing lint rules
- dependency warnings
- known runtime gaps
- current route count
- current packages/crates count
- current public exports count
```

---

# 1. Source-of-truth validation

Your architecture doc says every route must be statically declared in `ROUTE_TABLE`, and CI compares `--dump-routes` against docs. Good idea. Now weaponize it.

Run:

```bash
make verify-routes-doc
cargo run -p agent-gateway -- --dump-routes > /tmp/routes.md
diff -u docs/arch.md /tmp/routes.md
```

Review:

```txt
apps/backend/crates/agent-gateway/src/routes/**
apps/backend/crates/agent-gateway/src/main.rs
apps/backend/crates/agent-gateway/src/state.rs
docs/arch.md
```

Find gaps:

```txt
- routes documented but not wired
- routes wired but not documented
- admin routes missing auth middleware
- protected routes missing tenant middleware
- billing routes missing quota/metering
- upload routes missing size limits
- SSE/WebSocket routes missing timeout/cancellation behavior
```

Red flag from the doc: route documentation is treated as source-of-truth, but it is still manually coupled to route registration. That is fragile. The correct target is one route registry that generates both router wiring and docs. Two lists are just bugs waiting for coffee.

---

# 2. Dependency modernization audit

Your version matrix includes SvelteKit 2, Svelte 5, Tauri 2, Axum 0.8, Tailwind v4, Bits UI, shadcn-svelte, Rig 0.36, wasmtime 44, redb 2, Qdrant, Lago, Zitadel, and RustFS. That is a lot of moving glass.

## Frontend docs baseline

Use official docs as the upgrade reference, not blog posts from someone’s “ultimate 2026 stack” content farm.

Svelte 5 changed reactivity significantly, so audit runes usage, old store patterns, lifecycle assumptions, and component event syntax against the official migration guide. ([svelte.dev][1])

shadcn-svelte has a dedicated Svelte 5 migration path and explicitly separates shadcn-svelte migration from Bits UI migration, so audit those separately. Do not mix them into one “UI cleanup” blob unless you enjoy regression soup. ([shadcn-svelte][2])

Tauri 2 has its own v2 docs and updater/configuration model; audit `tauri.conf.json`, plugin permissions, mobile targets, updater strategy, CSP, and capability files against official Tauri v2 docs. ([Tauri][3])

Axum should be checked against latest docs for handler signatures, extractors, middleware layering, router composition, and tower integration. ([Docs.rs][4])

## Run package version check

```bash
pnpm outdated -r
pnpm audit
cargo outdated --workspace
cargo audit
cargo deny check
```

Add if missing:

```bash
cargo install cargo-outdated cargo-audit cargo-deny
pnpm add -D knip
```

Deliverable:

```txt
docs/audit/dependency-upgrade-matrix.md
```

Columns:

```txt
Package/crate | Current | Latest | Risk | Breaking changes | Owner | Decision
```

Decision options:

```txt
upgrade now
upgrade later
pin intentionally
remove
replace
```

---

# 3. Dead code detection

## Rust

Use three layers. One tool will lie. Several tools lie less.

```bash
cargo machete --with-metadata
cargo +nightly udeps --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

`cargo-machete` is fast but intentionally imprecise for unused dependency detection. Use it as a first pass, not a judge. ([GitHub][5])

`cargo-udeps` is slower and requires nightly, but it analyzes compiler output and is more accurate for unused dependency checks. ([GitHub][6])

Review dead-code categories:

```txt
- unused dependencies in Cargo.toml
- feature flags that are never enabled
- test-only code compiled into prod
- native providers registered nowhere
- manifests without runtime provider
- route handlers not mounted
- stores/traits with one implementation and no real abstraction value
- old migration jobs no longer runnable
- mock/in-memory code leaking into production binaries
```

Likely suspects from the architecture doc:

```txt
runtime-echo
template-wasm
storage-fs
legacy HMAC auth paths
convert-audio-to-text alias
capability-gaps-pan.md typo alias
build_output.txt
old docs/plan variants
```

Do not delete blindly. Mark each as:

```txt
DELETE
KEEP_DEV_ONLY
MOVE_TO_EXAMPLES
KEEP_PRODUCTION
UNKNOWN_NEEDS_OWNER
```

## TypeScript/Svelte

Install and run Knip:

```bash
pnpm add -D knip
pnpm knip
pnpm knip --production
```

Knip is specifically designed to find unused dependencies, exports, and files in JS/TS monorepos. That is exactly the disease you’re trying to diagnose. ([Knip][7])

Also run:

```bash
pnpm -r check
pnpm -r lint
pnpm exec biome check .
pnpm exec biome lint .
```

Biome has a fixable `noUnusedImports` rule, so unused imports should be automated, not manually hunted like it is 2014. ([Biome][8])

Review:

```txt
apps/web/src/**
apps/browser-shell/src/**
packages/ui/src/**
packages/sdk/src/**
packages/types/src/**
```

Find:

```txt
- components not imported anywhere
- duplicate app-specific stores
- stale Svelte 4 syntax
- unused exports from packages/ui
- duplicated UI between web and shell
- unused route files
- generated types checked in but stale
- old lucide package duplication
```

Specific red flag: the doc lists both `lucide-svelte` and `@lucide/svelte` in frontend dependencies. That smells like dependency drift. Pick one. Two icon libraries for the same thing is not “flexibility,” it is entropy with SVGs.

---

# 4. Backend architecture review

## 4.1 `agent-gateway`

Files:

```txt
apps/backend/crates/agent-gateway/src/main.rs
apps/backend/crates/agent-gateway/src/state.rs
apps/backend/crates/agent-gateway/src/routes/**
apps/backend/crates/agent-gateway/src/mw/**
```

Review checklist:

```txt
- AppState initialization order
- fallbacks when env vars are missing
- production behavior when billing provider is None
- route groups and middleware order
- request IDs propagated into every error
- body-size limits per route
- timeout/cancellation for LLM, MCP, WASM, RustFS, Qdrant
- graceful shutdown for cron jobs and streaming requests
- CORS allowlist not too broad in production
```

Required improvement:

```txt
Add startup validation mode:
cargo run -p agent-gateway -- validate-config
```

It should fail fast on:

```txt
- missing JWT/UI/session secrets in prod
- RUSTFS_IAM_ENC_KEY absent when per-tenant IAM is on
- webhook secrets missing
- LAGO_API_KEY missing while billing routes are enabled
- SUPER_ADMIN_EMAILS empty in prod
- Qdrant dimension mismatch
- unsupported LLM model alias
```

## 4.2 `agent-core`

Files:

```txt
agent/
capabilities/
chains/
identity/
indexing/
llm/
memory/
store/
vector_store/
```

Review checklist:

```txt
- CapabilityProvider trait: too broad or stable?
- registry locking: Mutex<HashMap> contention risk
- hot reload atomicity
- JSON schema validation before tool execution
- MCP host allowlist actually enforced everywhere
- WASM sandbox memory/time limits
- chain prompt template injection risks
- tenant isolation at redb/Qdrant/S3 boundary
- Qdrant collection recreation behavior in production
- thread truncation strategy quality
- audit event coverage for all mutating operations
```

Big architectural risk: “Everything is a capability” is elegant until everything becomes a generic escape hatch. Every capability kind needs strict execution budgets:

```txt
max wall time
max payload bytes
max output bytes
max retries
max tool calls
max cost
tenant quota impact
```

If this does not exist centrally, add it.

## 4.3 Jobs

Files:

```txt
apps/backend/crates/jobs/src/**
```

Review:

```txt
- idempotency
- retry policy
- stuck task recovery
- cancellation
- cron overlap prevention
- task ownership by tenant
- progress SSE cleanup
- admin run-now auth
```

Add:

```txt
JobRunId
idempotency_key
started_at
heartbeat_at
cancel_requested_at
last_error
attempt_count
```

Without that, background jobs become where bugs go to grow a beard.

---

# 5. Frontend architecture review

## 5.1 SvelteKit web app

Files:

```txt
apps/web/src/hooks.server.ts
apps/web/src/lib/**
apps/web/src/routes/**
```

Audit against Svelte 5 and SvelteKit current docs:

```txt
- load function boundaries
- server/client leakage
- env usage
- session cookie handling
- CSRF handling
- form actions
- API proxy behavior
- error boundaries
- remote function usage if adopted
- streaming/SSE lifecycle cleanup
```

Run:

```bash
pnpm --filter web check
pnpm --filter web build
pnpm --filter web test
pnpm --filter web exec playwright test
```

Specific issue to inspect:

```txt
hooks.server.ts has CSRF/session/font preload responsibilities.
```

That file can easily become a junk drawer. Split if needed:

```txt
server/csrf.ts
server/session.ts
server/security-headers.ts
server/fonts.ts
```

## 5.2 Tauri shell

Files:

```txt
apps/browser-shell/src/**
apps/browser-shell/src-tauri/**
apps/browser-shell/src-tauri/tauri.conf.json
apps/browser-shell/src-tauri/capabilities/**
```

Audit:

```txt
- CSP correctness
- plugin permissions
- updater support
- mobile deep links
- secure token storage
- Stronghold usage
- local dev API base leakage into prod builds
- WebView platform differences
- iOS/Android permissions
- desktop updater disabled accidentally
```

Tauri v2 expects explicit plugin/capability thinking. If permissions are broad “because dev was annoying,” fix that. Dev convenience is not a security model; it is a confession.

---

# 6. Shared UI package review

Files:

```txt
packages/ui/src/**
docs/ui-*
```

Principles from your doc:

```txt
- cross-platform UI belongs in packages/ui
- no app-only stores
- no hardcoded design tokens
- reduced motion first-class
```

Audit commands:

```bash
pnpm --filter @conusai/ui check
pnpm --filter @conusai/ui test
pnpm exec biome check packages/ui
pnpm knip --workspace packages/ui
```

Manual review:

```txt
- every exported component has a real consumer or story/demo
- every store is cross-platform
- no browser-only API without adapter
- no raw hex/rgb outside token files
- animations respect prefers-reduced-motion
- shadcn-svelte components match current Svelte 5 / Tailwind 4 conventions
- Bits UI APIs are current
```

Delete or move:

```txt
- one-off components used once
- visual experiments
- old token files
- old component variants
- duplicated primitives
```

Target package structure:

```txt
packages/ui/src/lib/components/primitives
packages/ui/src/lib/components/app-shell
packages/ui/src/lib/components/workspace
packages/ui/src/lib/stores
packages/ui/src/lib/adapters
packages/ui/src/lib/tokens
packages/ui/src/lib/motion
```

---

# 7. SDK and OpenAPI review

Files:

```txt
packages/sdk
packages/types
scripts/openapi-to-types.sh
apps/backend/.../openapi
```

Review:

```txt
- generated types never manually edited
- SDK has no duplicated request/response types
- all routes have typed client wrappers or intentionally raw methods
- error envelope is typed once
- streaming APIs have typed event models
- auth/session behavior is not duplicated between app and SDK
```

Add CI gate:

```bash
pnpm --filter @conusai/types prebuild
git diff --exit-code packages/types
```

If generated types differ after build, CI fails. Otherwise “generated” means “generated when someone remembers,” which is just manual with better branding.

---

# 8. Security review

Priority files:

```txt
identity/**
mw/**
store/creds.rs
store/tenant_storage.rs
routes/files*
routes/uploads*
routes/workspaces*
billing_webhook*
internal/rustfs_events*
```

Checklist:

```txt
- no default secrets accepted in prod
- JWT alg pinned
- OIDC audience/issuer validated
- legacy auth impossible in prod unless explicitly allowed
- admin email allowlist required
- API keys hashed at rest if persisted
- webhook signatures use constant-time compare
- upload paths reject traversal/control bytes
- presigned URLs tenant-scoped
- RustFS fallback root disabled in prod
- Qdrant filters always include tenant_id for content
- audit log on every mutation
- billing webhook replay protection
```

Add tests:

```txt
tenant_cannot_read_other_tenant_workspace
tenant_cannot_download_other_tenant_object
super_admin_can_override_tenant_with_audit
api_key_maps_to_correct_tenant
webhook_rejects_bad_signature
legacy_auth_rejected_in_prod
```

---

# 9. Capability system review

Files:

```txt
apps/backend/capabilities/**
agent-core/src/capabilities/**
docs/capabilities/**
```

Audit every manifest:

```txt
name
namespace
kind
tools
input_schema
output_schema
cost_hint
accepts/emits
auth requirements
timeout
tenant visibility
enabled default
test fixture
```

Create:

```bash
cargo run -p xtask -- validate-capabilities
```

It should verify:

```txt
- every manifest parses
- every native kind has a provider
- every provider has a manifest
- every MCP endpoint is allowed
- every WASM file exists and is under size cap
- every chain has model alias
- every output_schema is valid JSON Schema
- every capability has at least one eval
```

Likely cleanup:

```txt
runtime-echo       -> move to examples/dev
template-wasm      -> move to examples/dev
storage-fs         -> dev-only or delete if workspace storage replaced it
convert-audio-to-text -> remove if only alias to transcribe-video
plan-orchestrate   -> keep only if actively used in agent runtime
```

---

# 10. Infra / Dokploy review

Files:

```txt
docker-compose.yml
dokploy/**
docker/**
scripts/**
```

Checklist:

```txt
- compose parity between local and prod
- secrets never committed
- .env.example complete
- healthchecks exist for every service
- migrations are explicit
- domain sync idempotent
- volume wipe scripts protected
- observability stack optional but documented
- Jaeger/metrics not public unless intentionally protected
- RustFS admin not exposed
- Zitadel bootstrap repeatable
- Lago migration order safe
```

Add:

```bash
pnpm epifly doctor
pnpm epifly diff
pnpm epifly verify
```

CI should run “diff” without mutation.

---

# 11. Documentation cleanup

The doc tree already shows clutter:

```txt
docs/capability-gaps-plan.md
docs/capability-gaps-pan.md
docs/branding/indes.html
build_output.txt
many overlapping plan docs
```

That is not harmless. Old docs are dead code with better grammar.

Classify docs:

```txt
SOURCE_OF_TRUTH
ADR
REFERENCE
HISTORICAL
DELETE
```

Rules:

```txt
- only one architecture reference
- only one current deployment guide
- old plans move to docs/archive/YYYY-MM
- typo aliases get deleted after links fixed
- generated logs never committed
```

Delete candidate list:

```txt
build_output.txt
docs/capability-gaps-pan.md
docs/branding/indes.html
stale docs/plan.md variants after merging useful content
```

---

# 12. Testing plan

## Rust

Add or enforce:

```bash
cargo test --workspace --all-features
cargo nextest run --workspace --all-features
cargo llvm-cov nextest --workspace --all-features
```

Test groups:

```txt
unit: schema, path safety, env parsing, auth parsing
integration: redb, Qdrant, RustFS, Lago mock, Zitadel mock
contract: OpenAI-compatible chat completions
tenant isolation: every store boundary
capability: manifest validation + invocation
job: idempotency + retry + cancellation
```

## Frontend

```bash
pnpm -r check
pnpm -r test
pnpm --filter web exec playwright test
pnpm --filter web exec playwright test --project=reduced-motion
```

Test groups:

```txt
component
accessibility
visual regression
reduced motion
auth/session
workspace interactions
billing UI states
SSE reconnect
mobile shell smoke
```

---

# 13. CI gate proposal

Add one command:

```bash
just gate
```

It should run:

```bash
pnpm install --frozen-lockfile
pnpm -r check
pnpm -r lint
pnpm -r test
pnpm -r build
pnpm knip

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo audit
cargo deny check
cargo machete --with-metadata

make verify-routes-doc
```

Optional nightly lane:

```bash
cargo +nightly udeps --workspace --all-targets
```

Do not block every PR on nightly if it is flaky. Run it scheduled daily.

---

# 14. PR sequence

Do not do one mega-refactor PR. That is how codebases get murdered and nobody can identify the weapon.

## PR 1 — Baseline audit

```txt
- add audit docs
- add just gate
- add missing CI checks
- no behavior changes
```

## PR 2 — Dependency inventory

```txt
- add dependency upgrade matrix
- add cargo-deny
- add knip config
- add cargo-machete config
```

## PR 3 — Delete obvious junk

```txt
- remove committed build logs
- remove typo docs aliases after link fix
- archive stale plans
- remove unused exports/imports
```

## PR 4 — Frontend modernization

```txt
- Svelte 5 rune cleanup
- shadcn-svelte/Bits UI API alignment
- Tailwind v4/token cleanup
- remove duplicate icon package
```

## PR 5 — Backend safety hardening

```txt
- config validation
- production secret checks
- route/middleware test coverage
- timeout/cancellation defaults
```

## PR 6 — Capability cleanup

```txt
- validate all manifests
- move demo capabilities to examples
- delete unused aliases
- add capability eval fixtures
```

## PR 7 — Infra hardening

```txt
- Dokploy verify gates
- healthchecks
- secret checks
- observability protection
```

## PR 8 — Upgrade libraries

```txt
- upgrade low-risk packages first
- upgrade one major surface at a time
- keep lockfile diff reviewable
```

---

# 15. Definition of done

The cleanup is done only when these are true:

```txt
- zero unused TS/Svelte files unless ignored with reason
- zero unused TS exports unless public API with reason
- zero unused Rust dependencies unless ignored with reason
- all routes generated/verified from one source
- all generated OpenAPI types reproducible
- no committed build logs
- no typo duplicate docs
- no dev/demo capability active in prod
- all prod secrets validated at startup
- all mutating routes audited
- all tenant storage paths tested for isolation
- all frontend packages pass Svelte 5/shadcn-svelte current conventions
- dependency upgrade matrix exists and is reviewed
```

---

## Highest-priority fixes I would attack first

1. **Add production config validation.** Missing secrets/default dev modes are the kind of bug that turns into a postmortem with legal reviewing the adjectives.

2. **Run Knip + cargo-machete/cargo-udeps.** Dead code first. Updating dead code is just polishing a corpse.

3. **Remove duplicate frontend dependencies.** Especially the lucide duplication.

4. **Move demo capabilities out of production registry.** `runtime-echo` and `template-wasm` should not sit beside real tools unless clearly dev-gated.

5. **Make route registry generate docs and wiring.** Two manual route lists are not architecture. They are a synchronization bug with ceremony.

6. **Archive stale docs.** The repo already has too many “plan” files. Plans expire. Code does not care about your previous intentions.

[1]: https://svelte.dev/docs/svelte/v5-migration-guide?utm_source=chatgpt.com "Svelte 5 migration guide"
[2]: https://www.shadcn-svelte.com/docs/migration/svelte-5?utm_source=chatgpt.com "Svelte 5"
[3]: https://v2.tauri.app/plugin/updater/?utm_source=chatgpt.com "Updater"
[4]: https://docs.rs/axum/latest/axum/?utm_source=chatgpt.com "axum - Rust"
[5]: https://github.com/bnjbvr/cargo-machete?utm_source=chatgpt.com "bnjbvr/cargo-machete: Remove unused Rust ..."
[6]: https://github.com/est31/cargo-udeps?utm_source=chatgpt.com "est31/cargo-udeps: Find unused dependencies in Cargo.toml"
[7]: https://knip.dev/?utm_source=chatgpt.com "Knip: Declutter your JavaScript & TypeScript projects"
[8]: https://biomejs.dev/linter/rules/no-unused-imports/?utm_source=chatgpt.com "noUnusedImports - Biome.js"
