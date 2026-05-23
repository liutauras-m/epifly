/**
 * platform.ts — capability-precise platform detectors (Phase 5.1).
 *
 * All exports are pure functions evaluated at call-time (NOT cached at module
 * load) so they work in SSR and Tauri environments where `window` availability
 * may change between server render and client hydration.
 *
 * Naming convention: capability-first, not identity-first.
 *   ✓ isIOSWebView()        — "does this run on iOS?"
 *   ✓ supportsHaptics()     — "can I fire haptics?"
 *   ✗ isWeb()               — meaningless in Tauri (everything is web)
 *
 * Usage:
 *   import { isIOSWebView, supportsHaptics } from '@conusai/ui/utils/platform.js';
 *
 *   if (isIOSWebView()) { ... }
 *   if (!isTauriRuntime() && !isAndroidWebView()) { ... } // pure browser
 */

// ── Guards ────────────────────────────────────────────────────────────────────

function isBrowser(): boolean {
  return typeof window !== 'undefined' && typeof navigator !== 'undefined';
}

// ── Core detectors ────────────────────────────────────────────────────────────

/**
 * Returns true when code runs inside any Tauri WebView (all platforms).
 * Use this to branch on Tauri-specific APIs (clipboard, window, fs, etc.)
 */
export function isTauriRuntime(): boolean {
  return isBrowser() && '__TAURI_INTERNALS__' in window;
}

/**
 * Returns true when code runs inside an iOS Safari WebView or Tauri iOS WebView.
 * Use for iOS-specific styling and gesture behavior.
 */
export function isIOSWebView(): boolean {
  if (!isBrowser()) return false;
  return /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    // iPad on iOS 13+ reports as MacIntel with touch support
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
}

/**
 * Returns true when code runs on Android Chromium or Tauri Android WebView.
 */
export function isAndroidWebView(): boolean {
  if (!isBrowser()) return false;
  return /Android/.test(navigator.userAgent);
}

/**
 * Returns true when code runs on macOS (Tauri desktop or browser).
 * Distinguishes macOS from iOS — `isMacOSDesktop()` returns false in the iOS sim.
 */
export function isMacOSDesktop(): boolean {
  if (!isBrowser()) return false;
  // navigator.platform is deprecated but still the most reliable for macOS detection
  return /MacIntel/.test(navigator.platform) && navigator.maxTouchPoints === 0;
}

/**
 * Returns true when code runs on Windows (Tauri desktop or browser).
 */
export function isWindowsDesktop(): boolean {
  if (!isBrowser()) return false;
  return /Win/.test(navigator.platform);
}

/**
 * Returns true when code runs on Linux (Tauri desktop or browser).
 */
export function isLinuxDesktop(): boolean {
  if (!isBrowser()) return false;
  return /Linux/.test(navigator.platform) && !isAndroidWebView();
}

/**
 * Returns the canonical platform identifier.
 * Prefer capability predicates over this for logic branches.
 */
export function getPlatform(): 'web' | 'ios' | 'android' | 'macos' | 'windows' | 'linux' {
  if (!isBrowser())      return 'web';
  if (isIOSWebView())    return 'ios';
  if (isAndroidWebView()) return 'android';
  if (isMacOSDesktop())  return 'macos';
  if (isWindowsDesktop()) return 'windows';
  if (isLinuxDesktop())  return 'linux';
  return 'web';
}

// ── Capability predicates ─────────────────────────────────────────────────────

/**
 * Returns true when haptics can be fired.
 * Tauri haptics plugin takes priority; falls back to `navigator.vibrate`.
 */
export function supportsHaptics(): boolean {
  if (!isBrowser()) return false;
  // Tauri haptics plugin (checked at runtime — plugin may not be registered yet)
  if (isTauriRuntime()) return true;   // assume present; haptics.ts will no-op if not
  return typeof navigator.vibrate === 'function';
}

/**
 * Returns true when `env(safe-area-inset-*)` provides non-zero values.
 * Reliable only after layout; call inside onMount or $effect.
 */
export function supportsSafeAreaEnv(): boolean {
  if (!isBrowser()) return false;
  // Quick heuristic: iOS always has non-zero top inset
  return isIOSWebView() || isAndroidWebView();
}

/**
 * Returns true when the View Transitions API is available.
 */
export function supportsViewTransitions(): boolean {
  return isBrowser() && 'startViewTransition' in document;
}

/**
 * Returns true when the Web Share API is available.
 */
export function supportsWebShare(): boolean {
  return isBrowser() && 'share' in navigator;
}

// ── Pre-hydration script ──────────────────────────────────────────────────────

/**
 * Inline script content to set `data-platform` on `<html>` before hydration.
 * Inject via ThemeScript.ts pattern in app.html so first paint is correct.
 *
 * Usage in app.html (after the ThemeScript):
 *   <script>{@html PLATFORM_SCRIPT}</script>
 */
export const PLATFORM_SCRIPT = `(function(){
  var ua = navigator.userAgent;
  var p = navigator.platform || '';
  var t = navigator.maxTouchPoints || 0;
  var platform =
    (/iPad|iPhone|iPod/.test(ua) || (p === 'MacIntel' && t > 1)) ? 'ios' :
    /Android/.test(ua) ? 'android' :
    (/MacIntel/.test(p) && t === 0) ? 'macos' :
    /Win/.test(p) ? 'windows' :
    /Linux/.test(p) ? 'linux' : 'web';
  document.documentElement.dataset.platform = platform;
  if ('__TAURI_INTERNALS__' in window) {
    document.documentElement.dataset.tauri = 'true';
  }
})();`;
