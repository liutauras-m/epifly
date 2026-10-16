# ConusAI iOS Simulator — Install & Verification Plan

Install of the **`apps/browser-shell` Tauri 2 app** onto a macOS iOS Simulator, plus the Playwright iOS WebKit suite that exercises the same UI surface without Tauri. Companion to [`verify.md`](verify.md) (gateway / data plane) and [`verify-web.md`](verify-web.md).

> **Architecture under test**: `apps/browser-shell` (SvelteKit `adapter-static` + Tauri 2); Rust `browser_shell_lib` compiled for `aarch64-apple-ios-sim`; bundled into `ConusAI Browser.app`; installed on a booted iOS Simulator via `xcrun simctl`. The shell calls the gateway directly at the build-time `VITE_GATEWAY_URL` with `Authorization: Bearer <Zitadel access token>`. Sign-in is Rust-side Zitadel OIDC (authorization code + PKCE, `state`, `nonce`) through the system browser and a loopback redirect `http://127.0.0.1:<port>/callback` (`src-tauri/src/auth/`); the refresh token lives in a Stronghold vault keyed with Argon2 over a per-install salt, the access token only in memory. No token is stored in the webview.
>
> **Mobile sign-in is not implemented**: the loopback redirect and `open::that` system-browser launch are desktop mechanisms, so on the iOS simulator the shell shows its sign-in screen but cannot complete sign-in. Everything past the sign-in screen is verified through the Playwright `ios-mobile-web` project instead.
>
> The Playwright iOS suite (`e2e/ios/**`) uses the **`ios-mobile-web` project** — WebKit + iPhone 15 viewport (393 × 852 @ DPR 3) — against the `apps/web` SSR build on port 4173, signed in through the mock OIDC provider (`e2e/fixtures/mock-oidc.mjs`).

---

## Coverage Assessment

| Surface | Status | Notes |
|---|---|---|
| Tauri iOS scaffold (`src-tauri/gen/apple/`) | ✅ Initialised | `browser-shell.xcodeproj` + `project.yml` + `Podfile` committed |
| `tauri ios build --target aarch64-sim --debug` | ✅ Works | Produces `ConusAI Browser.app` (arm64 Mach-O); needs the three `VITE_*` build variables |
| `xcrun simctl install` / `launch com.conusai.browser` | ✅ Works | Launches into the sign-in screen |
| Sign-in on iOS / Android | ❌ Not implemented | Desktop loopback flow only |
| Native chat stream bridge (`chat_stream.rs` ↔ `tauri-stream.ts`) | ✅ Implemented | SSE from `/ui/stream` relayed as Tauri events, bearer token added by Rust |
| Refresh-token vault (Stronghold) | ✅ Implemented | Rust only; the Stronghold JS API is not registered |
| Playwright `ios-mobile-web` project | ✅ Runs in CI | `e2e/ios/features.spec.ts`, `e2e/ios/responsive.spec.ts` |
| Playwright iOS business spec (live gateway) | ⏭ Gated | `GATEWAY_INTEGRATION_TEST=1`; no working live configuration (see Phase 8.2) |
| Appium XCUITest suites (`e2e/wdio/specs/ios/{native,verify,capabilities-tour}.spec.ts`) | ⏭ Skipped | `describe.skip` since 2026-10-16: they forged the removed session cookie; they need a test Zitadel user and a scripted sign-in |
| Physical device install (`.ipa` + provisioning profile) | ⚠️ Requires Apple Developer team id | `tauri.conf.json → bundle.iOS.developmentTeam` is `null` |

---

## Prerequisites

```bash
# Xcode 16.x with iOS 18 SDK
xcode-select -p                    # → /Applications/Xcode.app/Contents/Developer
xcodebuild -version                # → Xcode 16+

# CocoaPods (Tauri uses these for some plugin linkage)
which pod

# Tauri CLI 2 (pinned via apps/browser-shell/package.json)
cd apps/browser-shell
pnpm exec tauri --version

# At least one booted iOS Simulator
xcrun simctl list devices booted

# Build-time configuration (public values, compiled into the app)
export VITE_GATEWAY_URL=http://127.0.0.1:8080          # the simulator reaches the host on 127.0.0.1
export VITE_ZITADEL_ISSUER=http://localhost:8085
export VITE_ZITADEL_NATIVE_CLIENT_ID=<native app client id>
```

`just shell-ios-dev` and `just shell-ios-build` set `VITE_GATEWAY_URL` themselves; the two
Zitadel values must come from the environment. The local gateway stack is `just up`
([`verify.md`](verify.md)).

---

## Phase 1 — Build the SvelteKit static bundle

The Tauri iOS bundle ships the pre-rendered SvelteKit static output (`apps/browser-shell/build/`) inside the `.app`. Rebuild it whenever the frontend changes.

```bash
cd apps/browser-shell
pnpm build

# Expect:
#   [vite-plugin-static-copy] Copied 21 items.
#   ✓ built in ~4s
#   ✔ done (Wrote site to "build")
ls build/index.html build/_app/    # static output present
```

The Tauri CLI invokes this automatically via `build.beforeBuildCommand` in `tauri.conf.json`, but running it standalone is the fastest feedback loop for UI-only changes.

---

## Phase 2 — Build the iOS simulator binary

```bash
cd apps/browser-shell
pnpm exec tauri ios build --target aarch64-sim --debug
```

What this does:

1. Re-runs the `beforeBuildCommand` (vite build).
2. Compiles `browser_shell_lib` for `aarch64-apple-ios-sim` (Rust target).
3. Runs `xcodebuild` against `src-tauri/gen/apple/browser-shell.xcodeproj` with the `iOS Simulator` SDK.
4. Bundles the static frontend, native libs, icons, and `Info.plist` into `ConusAI Browser.app`.

Build output locations:

```
src-tauri/gen/apple/build/browser-shell_iOS.xcarchive/Products/Applications/ConusAI Browser.app   # canonical fresh build
src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app                                          # CLI-managed install destination
```

### Known papercut — final rename failure

Tauri CLI 2.11.1 sometimes errors at the post-build rename step when a stale `arm64-sim/ConusAI Browser.app` directory is already present:

```
failed to rename app .../browser-shell_iOS.xcarchive/Products/Applications/ConusAI Browser.app:
  Directory not empty (os error 66)
```

The Xcode build itself succeeded — the `.app` inside the `xcarchive` is valid. Work-around:

```bash
APP_DST="src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app"
APP_SRC="src-tauri/gen/apple/build/browser-shell_iOS.xcarchive/Products/Applications/ConusAI Browser.app"
rm -rf "$APP_DST"
cp -R "$APP_SRC" "$APP_DST"
```

### Cosmetic warnings (non-blocking)

- `AppIcon.appiconset/AppIcon-83.5x83.5@2x.png is 168x168 but should be 167x167` — iPad icon size off by one pixel; fix when refreshing icons.
- `ld: warning: __eh_frame section too large (max 16MB)` — informational; runtime exception unwinding unaffected.
- `Run script build phase 'Build Rust Code' will be run during every build` — by design; Tauri always re-invokes cargo so changes pick up.

Verify the binary architecture:

```bash
file "src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app/ConusAI Browser"
# → Mach-O 64-bit executable arm64
```

---

## Phase 3 — Install + launch on the booted simulator

```bash
# Pick the booted simulator (or a specific UDID)
SIM_ID=$(xcrun simctl list devices booted | awk -F'[()]' '/Booted/ {print $2; exit}')
echo "Simulator: $SIM_ID"

APP_PATH="src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app"

# Make the Simulator window visible
open -a Simulator

# Install + launch
xcrun simctl install "$SIM_ID" "$APP_PATH"
xcrun simctl launch "$SIM_ID" com.conusai.browser
```

Expected output:

```
com.conusai.browser: 35777     ← pid (varies)
```

Confirm the bundle is registered:

```bash
xcrun simctl listapps "$SIM_ID" | grep -A1 "conusai"
# →
#   "com.conusai.browser" = {
#       ApplicationType = User;
#       CFBundleIdentifier = "com.conusai.browser";
#       CFBundleName = "ConusAI Browser";
#       ...
#   }
```

Capture a screenshot of the running app:

```bash
mkdir -p /tmp/ios-shell
xcrun simctl io "$SIM_ID" screenshot /tmp/ios-shell/launch.png
open /tmp/ios-shell/launch.png
```

✅ **Pass**: the sign-in screen renders — `ConusAI` brand mark, `"Enter the workshop."` heading and one `Sign in` button (`packages/ui/src/lib/features/ShellLoginScreen.svelte`). Tapping it cannot complete on iOS (see the note at the top).

---

## Phase 4 — Appium XCUITest harness

`e2e/wdio/` holds a WebdriverIO + Appium XCUITest harness (`pnpm appium`, `pnpm wdio:ios-native`, config `e2e/wdio/wdio.ios-native.conf.ts`). Its native-shell suites are skipped (`describe.skip`) because they signed in by forging the removed `conusai_session` cookie. They can return once there is a test Zitadel user and a scripted sign-in step. `e2e/wdio/specs/ios/safari.spec.ts` drives the web app in Mobile Safari and signs in through `e2e/wdio/fixtures/login.ts` (the BFF flow). Setup details: `e2e/wdio/README.md`.

---

## Phase 5 — Live dev mode (hot reload)

```bash
cd apps/browser-shell
pnpm exec tauri ios dev "iPhone 16 Pro"
```

This:

1. Boots the requested simulator if needed.
2. Starts the Vite dev server on `:5174` (per `tauri.conf.json → build.devUrl`).
3. Builds + installs a fresh `.app` with `devUrl` baked in.
4. Streams `cargo` rebuilds back into the running app.

The Tauri CLI auto-sets `TAURI_DEV_HOST` if needed so the simulator can reach the host machine.

---

## Phase 6 — Stream device logs

The shell uses `tracing_subscriber` for in-process logs. To see them while the app runs:

```bash
xcrun simctl spawn "$SIM_ID" log stream \
  --predicate 'process == "ConusAI Browser"' \
  --level=debug
```

Use `--level=info` for terser output; switch to `error` to spot crashes only.

---

## Phase 7 — Re-launch and uninstall

```bash
# Bring the app back to foreground without rebuilding
xcrun simctl launch "$SIM_ID" com.conusai.browser

# Send to background (useful when scripting)
xcrun simctl terminate "$SIM_ID" com.conusai.browser

# Wipe state — clears the Stronghold vault, web view storage, NSUserDefaults
xcrun simctl uninstall "$SIM_ID" com.conusai.browser
```

---

## Phase 8 — Playwright iOS WebKit suite (no simulator required)

The Playwright `ios-mobile-web` project runs WebKit with the iPhone 15 viewport against the SvelteKit Node SSR build. It exercises the same `@conusai/ui` components the Tauri shell ships, without spinning up Tauri.

### 8.1 Mock suite (default)

```bash
pnpm --filter web build
pnpm e2e:ios                       # = playwright test --project=ios-mobile-web
```

Covers sign-in through the mock IdP, the signed-out page, workspace elements, SSE chat stream, file upload and responsive layout at 393 px. Screenshots land in `test-results/`.

### 8.2 Live gateway use cases

`e2e/ios/capabilities-business.spec.ts` (UC1–UC5) self-skips unless `GATEWAY_INTEGRATION_TEST=1` is set; its registry check also needs `SUPER_TOKEN`, a Zitadel access token of a `super_admin` user. The Playwright web server is wired to the mock IdP and to no gateway (`playwright.config.ts`), so the use cases have no working live configuration at present.

| UC | Domain | Capabilities exercised |
|----|--------|------------------------|
| UC1 | Finance — invoice processing | `extract.fields.invoice` (mandatory); `plan.orchestrate`, `storage.put`, `compose.report_md` (soft) |
| UC2 | Legal — contract review | `extract.fields.contract` (mandatory); `sense.classify_document`, `compose.report_md` (soft) |
| UC3 | Healthcare — medical claim | `extract.fields.medical_claim` ∥ `extract.ocr.vision` ∥ `ocr-service` (one of) |
| UC4 | HR — 8-CV screening | `extract.fields.cv` (mandatory); `compose.email`, `plan.orchestrate` (soft) |
| UC5 | Operations — incident + photos | extractor card OR graceful "Could not process image" error |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `failed to rename app: Directory not empty (os error 66)` | Stale `arm64-sim/ConusAI Browser.app` from a previous build | `rm -rf` the destination, then `cp -R` the fresh `.app` from `browser-shell_iOS.xcarchive/Products/Applications/` |
| `xcrun simctl install` errors with `Failed to load the simulator runtime` | Booted simulator's OS not installed | `xcrun simctl runtime list`; install via Xcode → Settings → Components |
| App launches but shows blank screen | SvelteKit static build missing | `cd apps/browser-shell && pnpm build` then re-run `tauri ios build` |
| App launches but cannot reach the gateway | Gateway not up, or `VITE_GATEWAY_URL` wrong at build time | `curl -s http://localhost:8080/health` first; rebuild with `VITE_GATEWAY_URL=http://127.0.0.1:8080` |
| `this build has no VITE_ZITADEL_ISSUER; rebuild with it set` | Build-time variable missing | Export the three `VITE_*` variables and rebuild |
| `tauri ios dev` cannot connect to dev server | Simulator can't reach host on `localhost:5174` | Tauri auto-sets `TAURI_DEV_HOST`; if it doesn't, set it manually to the host's LAN IP |
| App icon warning during build | iPad 83.5×83.5@2x is 168×168 (should be 167×167) | Regenerate icons; non-blocking |

---

## Quick Reference — one-liner re-install

After UI or Rust changes:

```bash
cd apps/browser-shell && \
  pnpm exec tauri ios build --target aarch64-sim --debug && \
  APP_SRC="src-tauri/gen/apple/build/browser-shell_iOS.xcarchive/Products/Applications/ConusAI Browser.app" && \
  APP_DST="src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app" && \
  rm -rf "$APP_DST" && cp -R "$APP_SRC" "$APP_DST" && \
  SIM_ID=$(xcrun simctl list devices booted | awk -F'[()]' '/Booted/ {print $2; exit}') && \
  xcrun simctl install "$SIM_ID" "$APP_DST" && \
  xcrun simctl launch "$SIM_ID" com.conusai.browser
```

---

## §17 Known bug — Tool-card UI does not render on mobile shell (Svelte 5 reactivity gap)

**Discovered:** 2026-05-21 on the iPhone 16 Pro simulator (Appium, before the shell moved to Zitadel sign-in).

**Symptom:** when the agent invokes a tool, the `<ToolCallCard>` chip does not render in the mobile chat view although the data path is intact: the gateway emits `tool_call_start` / `tool_call_result` on `/ui/stream`, `chat_stream.rs` relays them, `tauri-stream.ts` turns them into `tool_start` / `tool_result` deltas, and `createChatStream` fills its `toolCards` map. The `{#each [...toolCards.entries()]}` block in `<AgentChatStream>` never iterates.

**Diagnosis:** Svelte 5 deep-state `Map` mutations exposed through a plain-object factory getter do not propagate to child component templates. Mutating with `.set()`, reassigning a new `Map`, and reading a `toolCardsVersion` counter in the getter all failed; `createChatStream` also publishes a flattened `toolCardsList`.

**Affected paths:** `packages/ui/src/lib/features/createChatStream.svelte.ts`, `packages/ui/src/lib/features/AgentChatStream.svelte`, `packages/ui/src/lib/features/screens/ChatScreen.svelte`.

**Suggested fix:** convert `createChatStream` to a Svelte 5 class with `$state` fields instead of a factory with getters, then re-enable V10.1 and V10.2 in `e2e/wdio/specs/ios/verify.spec.ts` (that suite also needs the scripted Zitadel sign-in described in Phase 4).

Users still see the tool result in the assistant's text; only the live status chip is missing.
