<svelte:options runes={true} />
<script lang="ts">
  /**
   * @deprecated Use Sheet (Phase 3.2) — deleted at Phase 4 close via ui:contracts gate.
   *
   * Migration shim: delegates to the canonical Sheet component.
   * Adapter props: open/onClose/title → open/onclose/title.
   */
  import type { Snippet } from 'svelte';
  import Sheet from './Sheet.svelte';

  let {
    open     = false,
    onClose,
    title,
    children,
  }: {
    open?:    boolean;
    onClose:  () => void;
    title?:   string;
    children: Snippet;
  } = $props();
</script>

<Sheet bind:open onclose={onClose} {title} label={title ?? 'Options'}>
  {#if children}
    {@render children()}
  {/if}
</Sheet>
