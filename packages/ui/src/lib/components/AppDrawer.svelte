<svelte:options runes={true} />
<script lang="ts">
  /**
   * @deprecated Use Drawer (Phase 3.2) — deleted at Phase 4 close via ui:contracts gate.
   *
   * Migration shim: delegates to the canonical Drawer component.
   * Adapter props: open/onClose → open/onclose.
   */
  import type { Snippet } from 'svelte';
  import Drawer from './Drawer.svelte';

  let {
    open     = false,
    onClose,
    children,
  }: {
    open?:    boolean;
    onClose:  () => void;
    children: Snippet;
  } = $props();
</script>

<Drawer bind:open onclose={onClose} label="Workspace navigation">
  {#if children}
    {@render children()}
  {/if}
</Drawer>
