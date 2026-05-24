<svelte:options runes={true} />
<script lang="ts">
  /**
   * AttachmentSheet — bottom-sheet for attaching files in the Composer (Phase 3.5).
   *
   * Migrated from apps/browser-shell/src/lib/mobile/parts/AttachmentSheet.svelte.
   * Now consumes the canonical <Sheet> primitive and semantic tokens.
   *
   * Usage:
   *   <AttachmentSheet
   *     open={attOpen}
   *     onclose={() => attOpen = false}
   *     onAdd={handleAdd}
   *     onUpload={handleUpload}
   *   />
   */
  import Sheet from '../components/Sheet.svelte';
  import type { Attachment } from '../components/Composer.svelte';

  let {
    open,
    onclose,
    onAdd,
    onUpload,
  }: {
    open: boolean;
    onclose: () => void;
    onAdd: (atts: Attachment[]) => void;
    onUpload: (files: File[]) => Promise<Attachment[]>;
  } = $props();

  let fileInput: HTMLInputElement | undefined = $state();

  async function handleFiles(files: FileList | null) {
    if (!files || files.length === 0) return;
    const result = await onUpload(Array.from(files));
    if (result.length) { onAdd(result); onclose(); }
  }
</script>

<Sheet {open} {onclose} label="Add attachment">
  {#snippet children()}
    <div class="att-list">
      <button class="att-row" onclick={() => fileInput?.click()}>
        <svg
          viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
          width="22" height="22" aria-hidden="true"
        >
          <path d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/>
        </svg>
        <span>Choose file</span>
      </button>
      <input
        bind:this={fileInput}
        type="file"
        multiple
        style="display:none"
        onchange={(e) => handleFiles((e.target as HTMLInputElement).files)}
      />
    </div>
  {/snippet}
</Sheet>

<style>
  .att-list {
    display:        flex;
    flex-direction: column;
  }

  .att-row {
    display:       flex;
    align-items:   center;
    gap:           var(--space-4);
    height:        var(--hit, 56px);
    padding:       0 var(--space-5);
    border:        none;
    border-bottom: 1px solid var(--color-border);
    background:    none;
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-body);
    color:         var(--color-fg);
    cursor:        pointer;
    width:         100%;
    text-align:    left;
    transition:    background var(--duration-fast) var(--ease-standard);  /* [feedback] hover confirmation */
  }

  .att-row:hover { background: var(--color-bg-raised); }

  .att-row:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  @media (prefers-reduced-motion: reduce) {
    .att-row { transition: none; }
  }
</style>
