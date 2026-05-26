<svelte:options runes={true} />
<script lang="ts">
  /**
   * Drawer — edge-slide modal (Phase 3.2).
   *
   * Backed by native <dialog> for correct focus-trap and backdrop.
   * Used for the primary navigation rail on compact/mobile breakpoints.
   *
   * Features:
   *   - Left or right edge slide
   *   - Focus trap via native <dialog>
   *   - Keyboard: Escape closes
   *   - Click-outside (backdrop) closes
   *   - prefers-reduced-motion: instant show/hide (no translate)
   *   - Safe-area aware (--safe-left/right/top/bottom tokens)
   *   - aria-modal="true" + required aria-label (TypeScript-enforced)
   *
   * Usage:
   *   <Drawer bind:open={drawerOpen} label="Navigation">
   *     <Sidebar />
   *   </Drawer>
   */
  import type { Snippet } from 'svelte';
  import { prefersReducedMotion } from '../utils/motion-prefs.js';

  let {
    open      = $bindable(false),
    label,
    side      = 'left' as 'left' | 'right',
    width     = '280px',
    children,
    onclose,
  }: {
    open?:     boolean;
    /** Required — aria-label for the dialog landmark. */
    label:     string;
    side?:     'left' | 'right';
    width?:    string;
    children?: Snippet;
    onclose?:  () => void;
  } = $props();

  let dialogEl = $state<HTMLDialogElement | undefined>(undefined);

  // Open / close the native dialog
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
    // The backdrop is the <dialog> element itself — clicking outside the
    // .drawer-panel fires the event on the dialog.
    if (e.target === dialogEl) handleClose();
  }

  const reduced = prefersReducedMotion();
</script>

<dialog
  bind:this={dialogEl}
  class="drawer drawer-{side}"
  class:no-motion={reduced}
  style:--drawer-width={width}
  aria-label={label}
  aria-modal="true"
  onclose={handleClose}
  onclick={handleBackdropClick}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="drawer-panel"
    role="document"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    {#if children}
      {@render children()}
    {/if}
  </div>
</dialog>

<style>
  /* ── Dialog reset ────────────────────────────────────────────────────────── */
  .drawer {
    /* Reset UA <dialog> styles */
    padding:    0;
    margin:     0;
    border:     none;
    background: transparent;
    outline:    none;
    max-width:  none;
    max-height: none;
    width:      100%;
    height:     100%;
    overflow:   hidden;

    /* Backdrop */
    &::backdrop {
      background: var(--color-backdrop, rgba(0, 0, 0, 0.4));
      opacity:    0;
      transition: opacity var(--duration-normal) var(--ease-standard);  /* [continuity] */
    }

    &[open]::backdrop {
      opacity: 1;
    }
  }

  /* ── Panel ───────────────────────────────────────────────────────────────── */
  .drawer-panel {
    position:    absolute;
    top:         0;
    bottom:      0;
    width:       var(--drawer-width, 280px);
    background:  var(--color-bg-raised);
    overflow-y:  auto;
    overflow-x:  hidden;
    display:     flex;
    flex-direction: column;

    transition: transform var(--duration-normal) var(--ease-emphasized-decelerate);  /* [continuity] */
    will-change: transform;
  }

  /* ── Left drawer (LTR: slides from left; RTL: slides from right) ─────────── */
  .drawer-left .drawer-panel {
    left:         0;
    padding-left: var(--safe-left, 0px);
    border-right: 1px solid var(--color-border);
    transform:    translateX(-100%);
    box-shadow:   4px 0 24px var(--color-shadow-md, rgba(0,0,0,0.12));
  }
  .drawer-left[open] .drawer-panel {
    transform: translateX(0);
  }
  /* RTL: left-drawer becomes start-edge (right side) */
  :global([dir='rtl']) .drawer-left .drawer-panel {
    left:          auto;
    right:         0;
    padding-left:  0;
    padding-right: var(--safe-right, 0px);
    border-right:  none;
    border-left:   1px solid var(--color-border);
    transform:     translateX(100%);
    box-shadow:    -4px 0 24px var(--color-shadow-md, rgba(0,0,0,0.12));
  }
  :global([dir='rtl']) .drawer-left[open] .drawer-panel {
    transform: translateX(0);
  }

  /* ── Right drawer (LTR: slides from right; RTL: slides from left) ──────── */
  .drawer-right .drawer-panel {
    right:         0;
    padding-right: var(--safe-right, 0px);
    border-left:   1px solid var(--color-border);
    transform:     translateX(100%);
    box-shadow:    -4px 0 24px var(--color-shadow-md, rgba(0,0,0,0.12));
  }
  .drawer-right[open] .drawer-panel {
    transform: translateX(0);
  }
  /* RTL: right-drawer becomes start-edge (left side) */
  :global([dir='rtl']) .drawer-right .drawer-panel {
    right:        auto;
    left:         0;
    padding-right: 0;
    padding-left:  var(--safe-left, 0px);
    border-left:   none;
    border-right:  1px solid var(--color-border);
    transform:     translateX(-100%);
    box-shadow:    4px 0 24px var(--color-shadow-md, rgba(0,0,0,0.12));
  }
  :global([dir='rtl']) .drawer-right[open] .drawer-panel {
    transform: translateX(0);
  }

  /* ── Reduced motion ──────────────────────────────────────────────────────── */
  .no-motion .drawer-panel,
  .no-motion .drawer-panel {
    transition: none;
  }

  /* ── Safe area ───────────────────────────────────────────────────────────── */
  .drawer-panel {
    padding-top:    var(--safe-top,    0px);
    padding-bottom: var(--safe-bottom, 0px);
  }
</style>
