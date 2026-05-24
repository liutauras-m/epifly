<svelte:options runes={true} />
<script lang="ts">
  /**
   * LiveAnnouncer — screen-reader-only ARIA live region (Phase 4.10).
   *
   * Announces toast messages to assistive technology via `aria-live="polite"`.
   * Visual toast rendering is handled by `<ToastHost>` — this component is
   * the accessible counterpart that speaks to screen readers.
   *
   * Usage (pair with ToastHost in +layout.svelte):
   *   <ToastHost />
   *   <LiveAnnouncer />
   */
  import { toasts } from '../stores/toast.svelte.js';
</script>

<!--
  Screen-reader-only live region.
  aria-atomic="false" so individual toast additions are read one at a time.
  Visually hidden via CSS — content still read by AT.
-->
<div class="sr-only" aria-live="polite" aria-atomic="false" aria-relevant="additions">
  {#each toasts.items as toast (toast.id)}
    <span>{toast.message}</span>
  {/each}
</div>

<style>
  /* Visually hidden — accessible to screen readers */
  .sr-only {
    position: absolute;
    width:    1px;
    height:   1px;
    padding:  0;
    margin:   -1px;
    overflow: hidden;
    clip:     rect(0, 0, 0, 0);
    white-space: nowrap;
    border:   0;
  }
</style>
