# ConusAI Platform

A production-grade multitenant AI agent platform built with Rust + Rig (backend) and SvelteKit (frontend).

## Cross-platform shell

The **ConusAI Browser Shell** is a native desktop and mobile application (built with Tauri 2) that embeds the ConusAI agent interface on macOS, Windows, iOS, and Android. It supports device-token provisioning, session recording, dry-run replay, and offline-capable operation — all without requiring a separate browser. See [docs/browser-shell-plan.md](docs/browser-shell-plan.md) for the architecture and implementation roadmap, and [docs/browser-shell-user-guide.md](docs/browser-shell-user-guide.md) for end-user installation and usage instructions.

## Quick start

```bash
# Infrastructure only (Qdrant + RustFS)
./start.sh infra

# Full stack
./start.sh full
```

See [docs/arch.md](docs/arch.md) for the full architecture reference.

## How to add a new domain

Every new domain element (extraction, transformation, classification, delivery, etc.) requires **zero changes** to `agent-core` or `agent-gateway` — only a new manifest directory:

1. **Write a manifest** — create `apps/backend/capabilities/<your-cap>/capability.toml` following [docs/capability-authoring-guide.md](docs/capability-authoring-guide.md). Pick a namespace from the taxonomy in [docs/capabilities/taxonomy.md](docs/capabilities/taxonomy.md).

2. **Add a prompt template** (if `kind = "chain"`) — create `capabilities/<your-cap>/prompts/system.md` and reference it in the manifest's `chain.system_prompt` path field.

3. **Lint the manifest** — run `cargo xtask capabilities lint` to validate schema, taxonomy compliance, required fields, and `accepts`/`emits` consistency.

4. **Reload** — the `ManifestWatcher` picks up the new directory within 250 ms and registers the capability without restarting the gateway. Verify with `GET /v1/capabilities` or `GET /admin/capabilities`.
