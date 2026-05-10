# ADR 008 — Multi-Platform Shell: Single `src-tauri/` for macOS, iOS, Windows, Android

**Status:** Accepted  
**Date:** 2026-05-10

## Context

Four platform targets are required. The question is whether to maintain separate Tauri crates per platform or a single unified crate.

## Decision

**Single `src-tauri/` crate** compiles for all four targets. Platform-specific code (entitlements plist, iOS privacy manifest, Android Gradle shim) lives in generated subdirectories (`gen/apple/`, `gen/android/`) created by `tauri ios init` / `tauri android init` and is not hand-maintained.

**Tauri 2 mobile maturity:** as of 2025, Tauri 2 mobile support (iOS and Android) reached stable via `tauri-cli` ≥ 2.1. WKWebView on iOS and Android WebView both support `initialization_script` and the `__TAURI__.core.invoke` bridge, which is the only JS↔Rust surface the recorder uses.

**Build hosts:**
- macOS universal binary + iOS IPA: `macos-14` GitHub runner
- Windows MSI: `windows-2022` GitHub runner  
- Android AAB/APK: `ubuntu-24.04` GitHub runner (NDK cross-compile)

## Consequences

- The recorder, tabs manager, Stronghold token vault, and OTel bridge are shared across platforms.
- Per-platform capability JSON files (`capabilities/main-capability.json`, `capabilities/ios-capability.json`) enumerate platform-specific Tauri plugin permissions.
- `minSdkVersion = 26` (Android 8+) enforces system WebView auto-update path, avoiding WebView2-style bootstrapper complexity on Android.
- Apple Developer Team ID, Azure Trusted Signing tenant, and Google Play upload key are external prerequisites; CI jobs that require them are gated on secrets availability.
