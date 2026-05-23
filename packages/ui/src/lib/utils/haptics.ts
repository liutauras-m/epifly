/**
 * haptics.ts — unified haptics API (Phase 5.3).
 *
 * Single API, three backends, in priority order:
 *   1. Tauri haptics plugin  (@tauri-apps/plugin-haptics) — iOS / Android native
 *   2. navigator.vibrate()   — Web / Android browser
 *   3. no-op                 — desktop or unsupported
 *
 * Usage:
 *   import { haptics } from '@conusai/ui/utils/haptics.js';
 *   haptics.tap();       // light tap — send, select
 *   haptics.success();   // success tick — capability completed
 *   haptics.warning();   // warning bump — error toast, quota near
 *   haptics.error();     // heavy bump — operation failed
 *
 * Tauri capability wiring (required before haptics work in the app):
 *   Add "haptics:default" to apps/browser-shell/src-tauri/capabilities/main.json
 *   Add the plugin Cargo dependency in apps/browser-shell/src-tauri/Cargo.toml
 *   Without these, the Tauri path silently falls back to navigator.vibrate.
 */

import { isTauriRuntime } from './platform.js';

// ── Tauri haptics types (dynamic import to avoid bundling the plugin when unused) ──

type TauriImpactFeedbackStyle = 'light' | 'medium' | 'heavy' | 'soft' | 'rigid';
type TauriNotificationFeedbackType = 'success' | 'warning' | 'error';

interface TauriHapticsPlugin {
  impactFeedback(opts: { style: TauriImpactFeedbackStyle }): Promise<void>;
  notificationFeedback(opts: { type: TauriNotificationFeedbackType }): Promise<void>;
  selectionFeedback(): Promise<void>;
}

// Lazy-loaded plugin reference — avoids importing Tauri in non-Tauri environments
let _tauriHaptics: TauriHapticsPlugin | null = null;

async function getTauriHaptics(): Promise<TauriHapticsPlugin | null> {
  if (!isTauriRuntime()) return null;
  if (_tauriHaptics) return _tauriHaptics;
  try {
    // Dynamic import — bundler tree-shakes when plugin is absent
    const mod = await import('@tauri-apps/plugin-haptics' as string);
    _tauriHaptics = mod as unknown as TauriHapticsPlugin;
    return _tauriHaptics;
  } catch {
    // Plugin not installed / not registered in capabilities — fall through
    return null;
  }
}

// ── Navigator.vibrate fallback ────────────────────────────────────────────────

function vibrateIfSupported(pattern: number | number[]): void {
  if (typeof navigator !== 'undefined' && typeof navigator.vibrate === 'function') {
    navigator.vibrate(pattern);
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

export const haptics = {
  /**
   * Light tap — use for: send message, select item, open menu.
   * Maps to: Tauri impact light / vibrate 10ms
   */
  async tap(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.impactFeedback({ style: 'light' }).catch(() => {});
    } else {
      vibrateIfSupported(10);
    }
  },

  /**
   * Selection changed — use for: switching tabs, chip selection.
   * Maps to: Tauri selectionFeedback / vibrate 6ms
   */
  async selection(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.selectionFeedback().catch(() => {});
    } else {
      vibrateIfSupported(6);
    }
  },

  /**
   * Success tick — use for: capability completed, file uploaded.
   * Maps to: Tauri notification success / vibrate [10, 50, 20]
   */
  async success(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.notificationFeedback({ type: 'success' }).catch(() => {});
    } else {
      vibrateIfSupported([10, 50, 20]);
    }
  },

  /**
   * Warning bump — use for: quota near limit, non-critical errors.
   * Maps to: Tauri notification warning / vibrate [15, 40, 15]
   */
  async warning(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.notificationFeedback({ type: 'warning' }).catch(() => {});
    } else {
      vibrateIfSupported([15, 40, 15]);
    }
  },

  /**
   * Error bump — use for: operation failed, form validation error.
   * Maps to: Tauri notification error / vibrate [20, 60, 20]
   */
  async error(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.notificationFeedback({ type: 'error' }).catch(() => {});
    } else {
      vibrateIfSupported([20, 60, 20]);
    }
  },

  /**
   * Medium impact — use for: drag-and-drop drop, sheet dismiss.
   * Maps to: Tauri impact medium / vibrate 20ms
   */
  async impact(): Promise<void> {
    const tauri = await getTauriHaptics();
    if (tauri) {
      await tauri.impactFeedback({ style: 'medium' }).catch(() => {});
    } else {
      vibrateIfSupported(20);
    }
  },
} as const;

export type HapticsAPI = typeof haptics;
