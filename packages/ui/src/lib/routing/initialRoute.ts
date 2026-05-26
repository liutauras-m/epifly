/**
 * Parse initial navigation state from URL params (web) or Tauri deep-link (shell).
 *
 * Returns an object with any of:
 *   ws     — workspace node ID (restores `?ws=<id>` param on web)
 *   thread — thread ID to open
 *   cap    — capability name to pre-select
 *
 * On web:   reads `window.location.search`
 * On Tauri: also tries @tauri-apps/plugin-deep-link for `conusai://` URLs.
 *           The Tauri plugin import is dynamic + guarded so the module remains
 *           safe to use in web-only builds that don't bundle the plugin.
 *
 * PR 3.C
 */

export interface InitialRoute {
  ws?: string;
  thread?: string;
  cap?: string;
}

export async function initialRoute(): Promise<InitialRoute> {
  const result: InitialRoute = {};

  // ── 1. Web URL search params ──────────────────────────────────────────────
  if (typeof window !== 'undefined') {
    const url = new URL(window.location.href);
    const ws     = url.searchParams.get('ws');
    const thread = url.searchParams.get('thread');
    const cap    = url.searchParams.get('cap');
    if (ws)     result.ws     = ws;
    if (thread) result.thread = thread;
    if (cap)    result.cap    = cap;
  }

  // ── 2. Tauri deep-link (conusai://) ──────────────────────────────────────
  // Only attempted when running inside Tauri; silently ignored otherwise.
  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  if (isTauri) {
    try {
      // Use new Function to hide this import from Vite's static analysis entirely.
      // @tauri-apps/plugin-deep-link is only present in the Tauri shell build;
      // this guard is only reached when `isTauri` is true at runtime.
      // eslint-disable-next-line @typescript-eslint/no-implied-eval
      const deepLink = await (new Function('s', 'return import(s)'))('@tauri-apps/plugin-deep-link') as typeof import('@tauri-apps/plugin-deep-link');
      // Preferred API in plugin-deep-link v2.
      const getCurrentUrl = (deepLink as { getCurrentUrl?: () => Promise<string | null> }).getCurrentUrl;
      // Backward-compatible fallback for older plugin builds that exposed getCurrent().
      const getCurrent = (deepLink as { getCurrent?: () => Promise<string[] | null> }).getCurrent;
      const deepUrl = getCurrentUrl
        ? await getCurrentUrl()
        : ((await getCurrent?.())?.[0] ?? null);
      if (deepUrl) {
        const deep = new URL(deepUrl);
        const ws     = deep.searchParams.get('ws');
        const thread = deep.searchParams.get('thread');
        const cap    = deep.searchParams.get('cap');
        // Deep-link params override URL params (more specific intent).
        if (ws)     result.ws     = ws;
        if (thread) result.thread = thread;
        if (cap)    result.cap    = cap;
      }
    } catch {
      // Plugin not installed or deep-link not present — non-fatal.
    }
  }

  return result;
}
