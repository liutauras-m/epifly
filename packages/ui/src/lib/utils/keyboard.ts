/**
 * keyboard.ts — global keyboard shortcut utilities (Phase 7).
 *
 * Registers app-wide keyboard shortcuts for keyboard-parity compliance.
 * All shortcuts documented in docs/ui-landmarks.md.
 *
 * Standard shortcuts:
 *   /        — focus composer (from any non-input element)
 *   Escape   — close any open drawer/sheet; blur composer if focused
 *   Cmd/Ctrl+K — open command palette (future)
 *
 * Usage (call once in +layout.svelte or AppShell $effect):
 *   import { registerKeyboardShortcuts } from '@conusai/ui/utils/keyboard.js';
 *   $effect(() => registerKeyboardShortcuts({ onFocusComposer, onEscape }));
 *
 * Each callback is optional — only registered shortcuts are active.
 */

export interface KeyboardShortcutHandlers {
  /** Called when `/` is pressed outside an input — should focus the composer */
  onFocusComposer?: () => void;
  /** Called when Escape is pressed — should close any open drawer/sheet */
  onEscape?: () => void;
  /** Called when Cmd/Ctrl+K is pressed — should open command palette */
  onCommandPalette?: () => void;
}

/**
 * Register global keyboard shortcuts.
 * Returns a cleanup function — call it in $effect return or onDestroy.
 */
export function registerKeyboardShortcuts(handlers: KeyboardShortcutHandlers): () => void {
  if (typeof window === 'undefined') return () => {};

  function onKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const inInput = target.tagName === 'INPUT'
      || target.tagName === 'TEXTAREA'
      || target.tagName === 'SELECT'
      || target.isContentEditable;

    console.log('[KEYBOARD UTILS] Key down on document:', e.key, 'target:', target.tagName, 'inInput:', inInput);
    // `/` — focus composer (not inside inputs)
    if (e.key === '/' && !inInput && !e.metaKey && !e.ctrlKey) {
      console.log('[KEYBOARD UTILS] "/" shortcut triggered! Calling onFocusComposer.');
      e.preventDefault();
      handlers.onFocusComposer?.();
      return;
    }

    // Escape — close drawer/sheet / blur composer
    if (e.key === 'Escape') {
      handlers.onEscape?.();
      return;
    }

    // Cmd/Ctrl+K — command palette
    if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handlers.onCommandPalette?.();
      return;
    }
  }

  document.addEventListener('keydown', onKeyDown);
  return () => document.removeEventListener('keydown', onKeyDown);
}

/**
 * Svelte action: focus the element when the `/` key is pressed.
 *
 * Usage:
 *   <textarea use:focusOnSlash />
 */
export function focusOnSlash(node: HTMLElement): { destroy(): void } {
  function onKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const inInput = target.tagName === 'INPUT'
      || target.tagName === 'TEXTAREA'
      || target.tagName === 'SELECT'
      || target.isContentEditable;

    if (e.key === '/' && !inInput && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      node.focus();
    }
  }

  document.addEventListener('keydown', onKeyDown);
  return { destroy() { document.removeEventListener('keydown', onKeyDown); } };
}
