export type AppMode = 'web' | 'shell';

let mode = $state<AppMode>('web');

export const modeStore = {
  get mode() { return mode; },
  setMode(m: AppMode) { mode = m; },
  get isShell() { return mode === 'shell'; },
  get isWeb() { return mode === 'web'; },
};
