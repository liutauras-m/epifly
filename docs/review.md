# Code Review & Cleanup Plan — Remaining Work

This file has been pruned to only the items not yet implemented. Items closed
out during the 2026-05 cleanup pass (gate expansion, `verify-routes-doc`,
`validate-config`, demo capability gating, machete cleanup, webhook/API-key
security tests, alias/doc deletions, etc.) have been removed.

---

## 0. Baseline evidence doc

Still missing the written baseline snapshot, even though the gate now passes
end-to-end.

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

## 1. Single source-of-truth route registry

`verify-routes-doc` now catches drift, but route docs and router wiring are
still two manually-maintained lists. Target: one registry that generates
both.

Find gaps still worth auditing:

```txt
- admin routes missing auth middleware
- protected routes missing tenant middleware
- billing routes missing quota/metering
- upload routes missing size limits
- SSE/WebSocket routes missing timeout/cancellation behavior
```

---

## 2. Dependency modernization audit

Frontend docs baseline — audit against official guides, not blog posts:

- Svelte 5 reactivity / runes / lifecycle / events.
- shadcn-svelte Svelte 5 migration (separate from Bits UI migration).
- Tauri 2 config, plugin permissions, updater, mobile targets, CSP,
  capability files.
- Axum latest handler/extractor/middleware conventions.

Run:

```bash
pnpm outdated -r
pnpm audit
cargo outdated --workspace
```

(`cargo audit` and `cargo deny check` already wired into the gate.)

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

## 3. Dead code detection — remaining lanes

`cargo machete` is clean, `pnpm knip --production` is clean, and
`cargo +nightly udeps --workspace --all-targets` is clean.

Remaining manual audit targets:

```txt
- unused TS/Svelte files
- unused exports from packages/ui
- duplicate UI between web and shell
- stale Svelte 4 syntax
- generated types checked in but stale
```

Each finding must be classified:

```txt
DELETE
KEEP_DEV_ONLY
MOVE_TO_EXAMPLES
KEEP_PRODUCTION
UNKNOWN_NEEDS_OWNER
```

---

## 4. Backend architecture review — remaining

### 4.1 agent-gateway

`validate-config` exists and prod legacy-auth is blocked. Still to verify:

```txt
- AppState initialization order under partial env
- route group + middleware order outside the verified admin tenant/auth path
- timeout/cancellation defaults for LLM, MCP, WASM, RustFS, Qdrant
- graceful shutdown for cron jobs and streaming requests
```

Extend `validate-config` to also fail on:

```txt
- Qdrant dimension mismatch
```

### 4.2 agent-core

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

Add central execution budgets for every capability kind:

```txt
max wall time
max payload bytes
max output bytes
max retries
max tool calls
max cost
tenant quota impact
```

### 4.3 Jobs

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

Add fields:

```txt
JobRunId
idempotency_key
started_at
heartbeat_at
cancel_requested_at
last_error
attempt_count
```

---

## 5. Frontend architecture review

### 5.1 SvelteKit web app

Audit against current Svelte 5 / SvelteKit docs:

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

Split `hooks.server.ts` if it has grown into a junk drawer:

```txt
server/csrf.ts
server/session.ts
server/security-headers.ts
server/fonts.ts
```

### 5.2 Tauri shell

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

---

## 6. Shared UI package review

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

## 7. SDK and OpenAPI review

```txt
- generated types never manually edited
- SDK has no duplicated request/response types
- all routes have typed client wrappers or intentionally raw methods
- error envelope is typed once
- streaming APIs have typed event models
- auth/session behavior not duplicated between app and SDK
```

CI gate is now wired through the OpenAPI generator script and enabled:

```bash
pnpm --filter @conusai/types prebuild
git diff --exit-code packages/types
```

---

## 8. Security review — remaining

Already covered: legacy-auth-rejected-in-prod, webhook signature
constant-time + reject-bad-signature (RustFS + Lago), api-key tenant
mapping, missing-header fall-through.

Still to harden / test:

```txt
- OIDC audience/issuer validated
- API keys hashed at rest if persisted
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
```

---

## 9. Capability system review

`xtask validate-capabilities` exists and demo manifests are gated. Still
to enforce per manifest:

```txt
cost_hint
accepts/emits
auth requirements
timeout
tenant visibility
enabled default
test fixture
```

Validator additions:

```txt
- every native kind has a provider
- every provider has a manifest
- every chain has model alias
- every output_schema is valid JSON Schema
- every capability has at least one eval
```

Cleanup decisions pending:

```txt
storage-fs         -> dev-only or delete if workspace storage replaced it
convert-audio-to-text -> remove if only alias to transcribe-video
plan-orchestrate   -> keep only if actively used in agent runtime
```

---

## 10. Infra / Dokploy review

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

Add to CI (diff only, no mutation):

```bash
pnpm epifly doctor
pnpm epifly diff
pnpm epifly verify
```

---

## 11. Documentation cleanup — remaining

Typo aliases and committed build logs are gone. Still to do:

Classify all remaining docs:

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
- generated logs never committed
```

Action: move stale `docs/*-plan.md` variants under `docs/archive/2026-05/`
after merging useful content into the canonical doc.

---

## 12. Testing plan — additions

Already running: `cargo test`, `pnpm -r test`, audit/deny/machete.

Add:

```bash
cargo nextest run --workspace --all-features
cargo llvm-cov nextest --workspace --all-features
pnpm --filter web exec playwright test --project=reduced-motion
```

Coverage groups still light:

```txt
contract: OpenAI-compatible chat completions
tenant isolation: every store boundary
capability: manifest validation + invocation (extend)
job: idempotency + retry + cancellation
accessibility
visual regression
SSE reconnect
mobile shell smoke
```

---

## 13. CI gate — optional nightly lane

`just gate` runs the main stages. Add a scheduled (not per-PR) lane:

```bash
cargo +nightly udeps --workspace --all-targets
```

---

## 14. PR sequence — remaining slices

PRs 1–3 (baseline / inventory / obvious junk) are effectively done in-tree.
Remaining:

```txt
PR 4 — Frontend modernization
  - Svelte 5 rune cleanup
  - shadcn-svelte/Bits UI API alignment
  - Tailwind v4 / token cleanup
  - remove duplicate icon package

PR 5 — Backend safety hardening (extend)
  - timeout/cancellation defaults
  - body-size limits per route
  - request-id propagation in error envelope

PR 6 — Capability cleanup
  - move demo capabilities to examples/
  - delete unused aliases
  - add capability eval fixtures

PR 7 — Infra hardening
  - Dokploy verify gates in CI
  - healthchecks
  - observability protection

PR 8 — Upgrade libraries
  - upgrade low-risk packages first
  - one major surface at a time
  - keep lockfile diff reviewable
```

---

## 15. Definition of done — outstanding

```txt
- zero unused TS/Svelte files unless ignored with reason
- zero unused TS exports unless public API with reason
- all routes generated from one source (not just verified)
- all generated OpenAPI types reproducible (CI diff gate)
- no dev/demo capability present in prod registry tree
- all mutating routes audited
- all tenant storage paths tested for isolation
- all frontend packages pass Svelte 5 / shadcn-svelte current conventions
- dependency upgrade matrix exists and is reviewed
```

---

## Highest-priority remaining

1. Knip + udeps cleanup.
2. Single source route registry that emits both wiring and docs.
3. Tenant-isolation integration tests across every store boundary.
4. Dependency upgrade matrix doc + scheduled `udeps` lane.
5. Archive stale `docs/*-plan.md` variants.
