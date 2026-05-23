<svelte:options runes={true} />
<script lang="ts">
  /**
   * Sheet — bottom-modal (Phase 3.2).
   *
   * Backed by native <dialog> for correct focus-trap and backdrop.
   * Used for: attachment picker, capability detail, profile options,
   * workspace create menu, any "peek" layer on mobile.
   *
   * On desktop (≥ 1024px) the sheet expands to a centered dialog instead
   * of a bottom-anchored sheet — same primitive, responsive presentation.
   *
   * Features:
   *   - Bottom slide on compact, centered dialog on expanded
   *   - Focus trap via native <dialog>
   *   - Keyboard: Escape closes
   *   - Click-outside closes
   *   - prefers-reduced-motion: instant open/close
   *   - Safe-area bottom padding
   *   - Required aria-label (TypeScript-enforced)
   *   - Optional title (renders <h2> inside sheet for VoiceOver rotor)
   *
   * Usage:
   *   <Sheet bind:open={sheetOpen} label="Attachment picker" title="Add attachment">
   *     <AttachmentList />
   *   </Sheet>
   */
  import type { Snippet } from 'svelte';
  import { prefersReducedMotion } from '../utils/motion-prefs.js';

  let {
    open     = $bindable(false),
    label,
    title,
    maxHeight = '85dvh',
    children,
    onclose,
  }: {
    open?:      boolean;
    /** Required — aria-label for the dialog landmark (or aria-labelledby when title= present). */
    label:      string;
    /** Optional visible heading — rendered as <h2> inside the sheet drag handle area */
    title?:     string;
    /** CSS max-height of the sheet panel. Default '85dvh'. */
    maxHeight?: string;
    children?:  Snippet;
    onclose?:   () => void;
  } = $props();

  let dialogEl = $state<HTMLDialogElement | undefined>(undefined);
  const titleId = $derived(title ? `sheet-title-${Math.random().toString(36).slice(2, 7)}` : undefined);

  $effect(() => {
    if (!dialogEl) return;
    if (open) {
      if (!dialogEl.open) dialogEl.showModal();
    } else {
      if (dialogEl.open) dialogEl.close();
    }
  });

  function handleClose() {
    open = false;
    onclose?.();
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === dialogEl) handleClose();
  }

  const reduced = prefersReducedMotion();
</script>

<dialog
  bind:this={dialogEl}
  class="sheet"
  class:no-motion={reduced}
  style:--sheet-max-height={maxHeight}
  aria-label={title ? undefined : label}
  aria-labelledby={titleId}
  aria-modal="true"
  onclose={handleClose}
  onclick={handleBackdropClick}
>
  <div
    class="sheet-panel"
    role="document"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Drag handle (visual only, decorative) -->
    <div class="sheet-handle" aria-hidden="true"></div>

    {#if title}
      <header class="sheet-header">
        <h2 id={titleId} class="sheet-title">{title}</h2>
      </header>
    {/if}

    <div class="sheet-body">
      {#if children}
        {@render children()}
      {/if}
    </div>
  </div>
</dialog>

<style>
  /* ── Dialog reset ────────────────────────────────────────────────────────── */
  .sheet {
    padding:     0;
    margin:      0;
    border:      none;
    background:  transparent;
    outline:     none;
    max-width:   none;
    max-height:  none;
    width:       100%;
    height:      100%;
    overflow:    hidden;

    &::backdrop {
      background: var(--color-backdrop, rgba(0, 0, 0, 0.4));
      opacity:    0;
      transition: opacity var(--duration-normal) var(--ease-standard);
    }
    &[open]::backdrop {
      opacity: 1;
    }
  }

  /* ── Sheet panel (bottom anchored) ──────────────────────────────────────── */
  .sheet-panel {
    position:       absolute;
    bottom:         0;
    left:           0;
    right:          0;
    max-height:     var(--sheet-max-height, 85dvh);
    background:     var(--color-bg-raised);
    border-radius:  var(--radius-xl) var(--radius-xl) 0 0;
    border-top:     1px solid var(--color-border);
    box-shadow:     0 -4px 32px var(--color-shadow-md, rgba(0,0,0,0.16));

    display:        flex;
    flex-direction: column;
    overflow:       hidden;

    padding-bottom: var(--safe-bottom, 0px);

    transform:      translateY(100%);
    transition:     transform var(--duration-normal) var(--ease-emphasized-decelerate);
    will-change:    transform;
  }

  .sheet[open] .sheet-panel {
    transform: translateY(0);
  }

  /* ── On expanded screens: centered dialog ───────────────────────────────── */
  @container app-shell (min-width: 1024px) {
    .sheet-panel {
      position:      relative;
      margin:        auto;
      left:          auto;
      right:         auto;
      bottom:        auto;
      width:         min(560px, 90cqi);
      max-height:    min(var(--sheet-max-height, 85dvh), 80dvh);
      border-radius: var(--radius-xl);
      border:        1px solid var(--color-border);
      transform:     scale(0.96) translateY(8px);
      opacity:       0;
      transition:
        transform var(--duration-normal) var(--ease-emphasized-decelerate),
        opacity   var(--duration-fast)   var(--ease-standard);
    }
    .sheet[open] .sheet-panel {
      transform: scale(1) translateY(0);
      opacity:   1;
    }
  }

  /* ── Reduced motion ──────────────────────────────────────────────────────── */
  .no-motion .sheet-panel {
    transition: none !important;
    transform:  none !important;
    opacity:    1 !important;
  }

  /* ── Drag handle ─────────────────────────────────────────────────────────── */
  .sheet-handle {
    width:        40px;
    height:       4px;
    background:   var(--color-border-strong);
    border-radius: var(--radius-full);
    margin:       var(--space-2) auto var(--space-1);
    flex-shrink:  0;
  }
  @container app-shell (min-width: 1024px) {
    .sheet-handle { display: none; }
  }

  /* ── Header ──────────────────────────────────────────────────────────────── */
  .sheet-header {
    padding:      var(--space-2) var(--space-5) var(--space-3);
    flex-shrink:  0;
    border-bottom: 1px solid var(--color-border);
  }
  .sheet-title {
    margin:       0;
    font-size:    var(--font-size-h2);   /* 20px */
    font-weight:  580;
    color:        var(--color-fg);
    letter-spacing: -0.016em;
  }

  /* ── Body ────────────────────────────────────────────────────────────────── */
  .sheet-body {
    flex:         1;
    overflow-y:   auto;
    overscroll-behavior: contain;
  }
</style>
