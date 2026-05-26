# Dependency Upgrade Matrix — 2026-05-26

Generated from:
- pnpm outdated -r
- pnpm audit
- cargo outdated --workspace (sampled output)
- cargo audit
- cargo deny check
- cargo machete --with-metadata

## JavaScript / TypeScript

| Package | Current | Latest | Risk | Breaking changes | Owner | Decision |
|---|---:|---:|---|---|---|---|
| svelte | 5.55.5 | 5.55.9 | Security advisory in <=5.55.6 (SSR XSS). | Patch/minor only. | web + browser-shell | upgrade now |
| @sveltejs/kit | 2.59.1 | 2.61.1 | Pulls devalue advisory chain in browser-shell. | Minor behavior checks for adapter/runtime. | web + browser-shell | upgrade now |
| vite | 6.4.2 | 8.0.14 | Multiple advisory chain items; also major jump available. | Major upgrade likely impacts plugins. | web + browser-shell + ui | upgrade later |
| vitest | 2.1.9 | 4.1.7 | Test infra lag; not prod runtime. | Major. | sdk + ui + web | upgrade later |
| vite-plugin-static-copy | 1.0.6 | 4.1.0 | Advisory flagged for <=2.3.1. | Major and plugin API drift likely. | browser-shell | upgrade now |
| @sveltejs/vite-plugin-svelte | 5.1.1 / 4.0.4 | 7.1.2 | Peer mismatch noise with current vite. | Major with vite coupling. | web + browser-shell + ui | upgrade later |
| @playwright/test | 1.59.1 | 1.60.0 | Low risk. | Minor. | qa/e2e | upgrade now |
| commander | 12.1.0 | 14.0.3 | CLI surface only. | Major but contained. | epifly | upgrade later |
| @clack/prompts | 0.9.1 | 1.4.0 | CLI UX dependency only. | Minor/major API checks. | epifly | upgrade later |
| typescript | 5.9.3 | 6.0.3 | Toolchain drift and type-rule changes. | Major; wide impact. | all TS owners | pin intentionally |
| jest | 29.7.0 | 30.4.2 | Test-only. | Major config changes likely. | epifly | upgrade later |
| @types/node | 22.19.19 | 25.9.1 | Type-only drift. | Potential ambient type breakage. | TS owners | pin intentionally |

## Rust

| Crate | Current | Latest/Fixed | Risk | Breaking changes | Owner | Decision |
|---|---:|---:|---|---|---|---|
| wasmtime-wasi | 44.0.2 | 44.0.2+ | RUSTSEC-2026-0149 was affecting 44.0.1. | Patch-level safe in current major. | backend core | upgrade now (completed) |
| glib | 0.18.5 | N/A in chain | RUSTSEC-2024-0429 unsound warning in Tauri GTK chain. | Upstream transitive through Tauri/wry. | browser-shell | upgrade later |
| unic-ucd-ident | 0.9.0 | none | RUSTSEC-2025-0100 unmaintained (transitive via urlpattern/tauri-utils). | No direct upgrade path. | browser-shell | pin intentionally |
| unic-ucd-version | 0.9.0 | none | RUSTSEC-2025-0098 unmaintained (same chain). | No direct upgrade path. | browser-shell | pin intentionally |
| tokio | 1.52.2 | 1.52.3 | Low risk patch bump appears repeatedly in cargo-outdated. | Patch. | backend core | upgrade now |
| axum-extra | 0.10.3 | 0.12.6 | Feature/API modernization opportunity. | Minor/major migration likely. | gateway | upgrade later |
| rand | 0.8.6 | 0.10.1 | Broad transitive movement. | Major in rand ecosystem. | backend core | upgrade later |

## Security Snapshot

- pnpm audit: 15 vulnerabilities (2 high, 12 moderate, 1 low).
- cargo audit: pass in CI baseline via explicit temporary ignores in `.cargo/audit.toml`.
- cargo deny: pass in CI baseline via workspace `deny.toml` (advisory ignore set + explicit license allowlist).

## Dead Dependency Snapshot

- cargo machete flagged candidates in:
  - apps/backend/crates/jobs/Cargo.toml
  - apps/backend/xtask/Cargo.toml
  - apps/browser-shell/src-tauri/Cargo.toml
- Action: verify each candidate before removal; do not auto-delete transitive-critical crates.

## Immediate Action Queue

1. Upgrade Svelte and SvelteKit patch/minor line to clear known advisories.
2. Upgrade vite-plugin-static-copy to >=2.3.2 (prefer latest stable after compatibility test).
3. Keep TypeScript 5.9 pinned for now; schedule a dedicated TS6 migration branch.
4. Track Tauri transitive unmaintained unic crates as accepted risk until upstream chain upgrades.
5. Resolve cargo-deny license policy failures (current dominant blocker).

## Evidence Files

- /tmp/audit_pnpm_outdated.log
- /tmp/audit_pnpm_audit.log
- /tmp/audit_cargo_outdated_sample.log
- /tmp/audit_cargo_audit_latest.log
- /tmp/audit_cargo_deny_latest.log
- /tmp/audit_cargo_machete.log
