export type ToastKind = 'info' | 'success' | 'error' | 'warning';

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

let nextId = 0;

export const toasts = (() => {
  let items = $state<Toast[]>([]);

  function add(message: string, kind: ToastKind = 'info', durationMs = 4000): number {
    const id = ++nextId;
    items = [...items, { id, message, kind }];
    if (durationMs > 0) setTimeout(() => dismiss(id), durationMs);
    return id;
  }

  function dismiss(id: number) {
    items = items.filter(t => t.id !== id);
  }

  return {
    get items() { return items; },
    add,
    dismiss,
    info:    (msg: string, ms?: number) => add(msg, 'info', ms),
    success: (msg: string, ms?: number) => add(msg, 'success', ms),
    error:   (msg: string, ms?: number) => add(msg, 'error', ms),
    warning: (msg: string, ms?: number) => add(msg, 'warning', ms),
  };
})();
