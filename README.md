# ConusAI Platform

A production-grade multitenant AI agent platform built with Rust + Rig (backend) and SvelteKit (frontend).

## Cross-platform shell

The **ConusAI Browser Shell** is a native desktop and mobile application (built with Tauri 2) that embeds the ConusAI agent interface on macOS, Windows, iOS, and Android. It supports device-token provisioning, session recording, dry-run replay, and offline-capable operation — all without requiring a separate browser. See [docs/browser-shell-plan.md](docs/browser-shell-plan.md) for the architecture and implementation roadmap, and [docs/browser-shell-user-guide.md](docs/browser-shell-user-guide.md) for end-user installation and usage instructions.

## Quick start

```bash
# Infrastructure only (Postgres + MinIO)
./start.sh infra

# Full stack
./start.sh full
```

See [docs/arch.md](docs/arch.md) for the full architecture reference.
