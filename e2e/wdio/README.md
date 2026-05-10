# Cross-platform E2E tests (WebdriverIO)

Layer beyond the Playwright suite. Drives the **native** ConusAI Browser app on real iOS simulators / devices and (planned) the Tauri desktop shell on macOS.

| Layer | Target | Driver | Status |
|---|---|---|---|
| L1 | iOS Safari (WebKit emulation) | Playwright | ✅ 23 tests in `e2e/ios/features.spec.ts` |
| L2 | macOS Tauri desktop shell | `tauri-webdriver` (debug-only crate) | ✅ scaffold ready, needs `cargo build` |
| L3 | iOS Simulator — native Tauri build | Appium XCUITest | ✅ 3 tests passing |
| L3 | iOS real device — same code | Appium XCUITest + WebDriverAgent | 🟡 ready, needs Apple Dev cert |
| L4 | Android emulator/device | Appium UiAutomator2 | ⏳ blocked on Tauri Android `init` |
| L4 | macOS Tauri build via Appium mac2-driver | Appium mac2 | ⏳ alternative to L2 |

## Quick start — iOS Simulator (no Apple cert)

```bash
# 1. Boot a simulator (one-time)
xcrun simctl boot "iPhone 16 Pro"
open -a Simulator

# 2. Build + install the native iOS Tauri app
pnpm ios:build         # cargo tauri ios build --target aarch64-sim --debug
pnpm ios:install       # xcrun simctl install booted "...ConusAI Browser.app"

# 3. Start Appium server (background)
pnpm appium &

# 4. Run native tests
IOS_DEVICE_UDID=<udid-from-simctl-list> pnpm wdio:ios-native
```

The simulator UDID is printed by `xcrun simctl list devices booted`.

## Real iOS device

Same test code, three extras:

1. **Apple Developer account** ($99/yr) and team membership.
2. **WebDriverAgent code-signing** — Appium ships `WebDriverAgent.xcodeproj`. Open in Xcode once, set the signing team, build for "My Mac", then build for your device. After that Appium handles it automatically.
3. **iOS Developer profile on the device** — connect via USB, trust the host, install the dev profile under Settings → General → VPN & Device Management.

```bash
# Build for real device (note: NOT -sim)
pnpm --filter browser-shell exec tauri ios build --target aarch64 --debug
# Install with ios-deploy or Xcode
ios-deploy --bundle "...build/arm64/ConusAI Browser.app"

# Run the same test against the real device
IOS_REAL_DEVICE=1 \
IOS_DEVICE_UDID=$(idevice_id -l | head -1) \
APPLE_TEAM_ID=ABCDE12345 \
WDA_BUNDLE_ID=com.yourorg.WebDriverAgentRunner \
pnpm wdio:ios-native
```

## Tests

- `specs/ios/native.spec.ts` — native iOS Tauri app, NATIVE_APP + WEBVIEW context.
- `specs/ios/safari.spec.ts` — mobile Safari hitting the SvelteKit web app (no native app needed).
- `specs/macos/shell.spec.ts` — macOS Tauri shell via `tauri-webdriver` (requires `cargo build -p browser-shell --features e2e` first).

## Appium driver hygiene

```bash
pnpm exec appium driver list           # see what's installed
pnpm exec appium driver doctor xcuitest # diagnose missing tools
```

Required tools for XCUITest: Xcode + Command Line Tools, `xcrun`, `idevice_id` (libimobiledevice — `brew install libimobiledevice`), `ios-deploy` (`brew install ios-deploy`).

## CI

For real-device CI without local cert pain, point `hostname`/`port` in the WDIO config at a cloud farm (BrowserStack, Sauce Labs, AWS Device Farm). Test code is unchanged — only capabilities differ.
