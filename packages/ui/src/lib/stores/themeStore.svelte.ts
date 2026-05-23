/**
 * themeStore.svelte.ts — theme store (Phase 5.5 update: system/paper/forge three-way).
 *
 * Theme resolution order:
 *   1. User-persisted preference in adapter (localStorage)
 *   2. OS prefers-color-scheme when preference is 'system'
 *   3. Default: 'paper'
 *
 * The store exposes `current` (resolved: 'paper' | 'forge') and
 * `preference` ('system' | 'paper' | 'forge') separately so ThemeSwitcher
 * can show the three-way state without needing to infer it.
 */

/** Resolved rendered theme — always 'paper' or 'forge' */
export type Theme = 'paper' | 'forge';

/** User preference — 'system' means follow prefers-color-scheme */
export type ThemePreference = 'system' | 'paper' | 'forge';

export interface ThemeAdapter {
  read(): ThemePreference;
  write(pref: ThemePreference): void;
}

export const localStorageAdapter: ThemeAdapter = {
  read() {
    if (typeof localStorage === 'undefined') return 'system';
    return (localStorage.getItem('conusai-theme') as ThemePreference) ?? 'system';
  },
  write(pref) {
    localStorage.setItem('conusai-theme', pref);
  },
};

/** Resolve the OS preferred color scheme */
function getSystemTheme(): Theme {
  if (typeof window === 'undefined') return 'paper';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'forge' : 'paper';
}

/** Apply the rendered theme to the DOM */
function applyTheme(theme: Theme) {
  if (typeof document === 'undefined') return;
  document.documentElement.setAttribute('data-theme', theme);
}

export function createThemeStore(adapter: ThemeAdapter = localStorageAdapter) {
  let preference = $state<ThemePreference>(adapter.read());

  /** Resolved rendered theme — 'system' is expanded to actual OS preference */
  const current = $derived<Theme>(
    preference === 'system' ? getSystemTheme() : preference
  );

  $effect(() => {
    applyTheme(current);

    // Listen for OS theme changes when user is in 'system' mode
    if (preference === 'system' && typeof window !== 'undefined') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const handler = () => { applyTheme(getSystemTheme()); };
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }
  });

  function setPreference(pref: ThemePreference) {
    preference = pref;
    adapter.write(pref);
  }

  /** Convenience: cycle through system → paper → forge */
  function toggle() {
    const next: ThemePreference =
      preference === 'system' ? 'paper'  :
      preference === 'paper'  ? 'forge'  : 'system';
    setPreference(next);
  }

  /** Legacy compat: direct theme set (bypasses 'system') */
  function set(theme: Theme) {
    setPreference(theme);
  }

  return {
    get current()    { return current; },
    get preference() { return preference; },
    set,
    setPreference,
    toggle,
  };
}

export type ThemeStore = ReturnType<typeof createThemeStore>;
