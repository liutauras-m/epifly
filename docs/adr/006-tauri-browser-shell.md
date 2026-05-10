# ADR 006 — Tauri 2 Browser Shell

**Status:** Accepted  
**Date:** 2026-05-10

## Context

ConusAI needs a cross-platform desktop/mobile shell that can record user browser sessions and upload them as `SessionTrace` artifacts to the workspace for replay by the agent. Four platforms are required: macOS, iOS, Windows, Android.

## Decision

**Tauri 2 over Electron.**  
Tauri produces binaries ~5–10× smaller than Electron (no bundled Chromium). The Rust core is shared with the existing `agent-core`/`agent-gateway` workspace, meaning the same `SessionTrace`, `CapabilityCard`, and `blake3` hash primitives are used without an FFI boundary or duplicated logic. Tauri 2 supports iOS and Android targets natively via `tauri ios init` / `tauri android init` backed by WKWebView and Android WebView respectively.

**SvelteKit 2 + Svelte 5 runes over Next.js.**  
The existing `apps/web` is already SvelteKit 2 + Svelte 5. Shared component library (`packages/ui`) can target both apps without a framework split. Svelte 5 produces a smaller runtime than React 19; runes replace the context/store boilerplate with zero-overhead reactivity.

**`adapter-static` for the shell, `adapter-node` for `apps/web`.**  
The Tauri shell bundles the SvelteKit output as a static site (`../build/`), requiring `adapter-static`. `apps/web` continues to use `adapter-node` for server-side rendering and SvelteKit form actions.

**Askama UI stays in parallel through v0.4.x.**  
The existing Askama-rendered admin pages remain unchanged to avoid blocking the shell feature. All new UI lands in `apps/web` and `packages/ui`. Askama is removed in v0.5.

## Consequences

- One `src-tauri/` Rust crate compiles for macOS, Windows, iOS, and Android; platform-specific code is minimal (`#[cfg(target_os = "...")]`).
- macOS build host required for Apple targets; any host for Windows (cross-compile via MSVC) and Android (NDK).
- The shell is a standard `remote_mcp` client; the backend sees it as any other capability consumer.
