/**
 * Minimal type shims for Tauri plugins that are only present in the
 * browser-shell (Tauri 2) build. These declarations allow `svelte-check` and
 * `tsc` to type-check files that reference these modules even when the packages
 * are not installed as devDependencies of @conusai/ui.
 *
 * The actual implementations are resolved at runtime by the Tauri bundler.
 */

declare module '@tauri-apps/plugin-deep-link' {
  /** Returns the deep-link URL that launched the app, or null if none. */
  export function getCurrentUrl(): Promise<string | null>;
}
