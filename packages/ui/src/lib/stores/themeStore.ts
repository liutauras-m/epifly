export type Theme = 'paper' | 'forge';

export interface ThemeAdapter {
  read(): Theme;
  write(theme: Theme): void;
}

export const localStorageAdapter: ThemeAdapter = {
  read() {
    if (typeof localStorage === 'undefined') return 'paper';
    return (localStorage.getItem('conusai-theme') as Theme) ?? 'paper';
  },
  write(theme) {
    localStorage.setItem('conusai-theme', theme);
  },
};

export function createThemeStore(adapter: ThemeAdapter = localStorageAdapter) {
  let current = $state<Theme>(adapter.read());

  function set(theme: Theme) {
    current = theme;
    adapter.write(theme);
    document.documentElement.setAttribute('data-theme', theme);
  }

  function toggle() {
    set(current === 'paper' ? 'forge' : 'paper');
  }

  return {
    get current() { return current; },
    set,
    toggle,
  };
}

export type ThemeStore = ReturnType<typeof createThemeStore>;
