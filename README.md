# ConusAI Platform

A production-grade multitenant AI agent platform built with Rust + Rig (backend) and SvelteKit (frontend).

## Cross-platform shell

The **ConusAI Browser Shell** is a native desktop and mobile application (built with Tauri 2) that embeds the ConusAI agent interface on macOS, Windows, iOS, and Android. It supports device-token provisioning, session recording, dry-run replay, and offline-capable operation — all without requiring a separate browser. See [docs/browser-shell-plan.md](docs/browser-shell-plan.md) for the architecture and implementation roadmap, and [docs/browser-shell-user-guide.md](docs/browser-shell-user-guide.md) for end-user installation and usage instructions.

## Quick start

```bash
scripts/generate-env.sh > .env && chmod 600 .env   # then set ANTHROPIC_API_KEY
just up                                            # whole stack in Docker
just web-dev                                       # optional: web with hot reload on :5173
```

`just up` runs `docker-compose.yml` (what ConusAI Cloud deploys) with
`docker-compose.local.yml` (Postgres, Redis, Qdrant and RustFS as local containers).

## Deploy

epifly deploys to **ConusAI Cloud** as one compose project with linked managed
Postgres, Redis, Qdrant and S3: see [docs/deploy-conusai-cloud.md](docs/deploy-conusai-cloud.md).

See [docs/arch.md](docs/arch.md) for the full architecture reference.

## How to add a new domain

Every new domain element (extraction, transformation, classification, delivery, etc.) requires **zero changes** to `agent-core` or `agent-gateway` — only a new manifest directory:

1. **Write a manifest** — create `apps/backend/capabilities/<your-cap>/capability.toml` following [docs/capability-authoring-guide.md](docs/capability-authoring-guide.md). Pick a namespace from the taxonomy in [docs/capabilities/taxonomy.md](docs/capabilities/taxonomy.md).

2. **Add a prompt template** (if `kind = "chain"`) — create `capabilities/<your-cap>/prompts/system.md` and reference it in the manifest's `chain.system_prompt` path field.

3. **Lint the manifest** — run `cargo xtask capabilities lint` to validate schema, taxonomy compliance, required fields, and `accepts`/`emits` consistency.

4. **Reload** — the `ManifestWatcher` picks up the new directory within 250 ms and registers the capability without restarting the gateway. Verify with `GET /v1/capabilities` or `GET /admin/capabilities`.

---

## Where to add UI

When writing frontend code, the **golden rule** is: `packages/ui` first, always. `apps/*` is for wiring only — data loading, routing, auth guards. Full guidelines are in [docs/ui-plan.md](docs/ui-plan.md) and [docs/ui-design.md](docs/ui-design.md).

```
Need UI?
│
├─ Is it reusable across screens with no domain coupling?
│  └─ YES → packages/ui/src/lib/components/  (primitives)
│     Examples: Button, Field, Chip, EmptyState, StatusBadge, Drawer
│     Rule: Props in, callbacks out. No store reads. No SDK calls.
│
├─ Does it compose primitives AND read workspace/billing/capability state?
│  └─ YES → packages/ui/src/lib/features/  (product UI)
│     Examples: WorkspaceTree, CapabilityBrowser, QuotaList
│     Rule: May use stores/ and capabilities/. No route ownership.
│
└─ Is it page-level wiring — layouts, data loading, auth guards?
   └─ YES → apps/web/src/routes/  or  apps/browser-shell/src/routes/
      Rule: No <style> blocks with color/font/radius values.
            If it grows a style block → extract a primitive.
```

**Naming rules (Principle #13 / #15 of ui-plan.md):**
- Components: `PascalCase.svelte` — `AppShell`, `MessageBubble`, `StatusBadge`
- Props: generic vocabulary — `variant="primary|secondary|ghost|danger"`, `size="sm|md|lg"`
- Never: `variant="ember"`, `<RailUserChip>`, brand names in component APIs
- CSS tokens: canonical long form — `--color-accent`, `--space-4`, `--radius-md`, `--duration-fast`

**Token rules (one token, one place):**
- No hex outside `packages/ui/src/lib/tokens.css` / `foundry.css`
- No `px` in layout props outside tokens (use `--space-*`, `--radius-*`, etc.)
- CI gate: `node scripts/check-design-tokens.mjs` fails on violations

**Primitive gallery:** `pnpm --filter web dev` → open [`/_/ui`](http://localhost:5173/_/ui) to see every primitive with fixtures.

**CI gate (run before every PR):**
```bash
pnpm ui:gates
# Runs: ui:contracts + design-tokens + motion:durations + motion:purpose + no-local + test:exports
# All must be green — zero exit code — before a PR merges.
```

| Gate script | What it enforces |
|---|---|
| `pnpm ui:contracts` | 8 architectural rules (brand scalars, viewport media, fixtures, font-variation-settings, raw element styling) |
| `pnpm ui:tokens:check` | No hex / raw px / cubic-bezier outside token files |
| `pnpm ui:motion:durations` | No animation > 400 ms (except cascade allowlist) |
| `pnpm ui:motion:purpose` | Every animation tagged `[feedback\|continuity\|hierarchy\|delight]` |
| `pnpm ui:no-local` | No app-local UI components (> 20 style lines outside routes) |
| `pnpm test:exports` | Every `@conusai/ui` export map entry resolves to a defined value |
