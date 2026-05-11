<script lang="ts">
  interface Item {
    id: string;
    label: string;
    description?: string;
    group?: string;
  }

  interface Props {
    open?: boolean;
    items?: Item[];
    placeholder?: string;
    onselect?: (id: string) => void;
    onclose?: () => void;
  }

  let { open = false, items = [], placeholder = "Search…", onselect, onclose }: Props = $props();

  let query = $state("");

  let filtered = $derived(
    query.trim().length === 0
      ? items
      : items.filter(
          (i) =>
            i.label.toLowerCase().includes(query.toLowerCase()) ||
            i.description?.toLowerCase().includes(query.toLowerCase())
        )
  );

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose?.();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => onclose?.()}></div>
  <div
    class="palette"
    role="dialog"
    aria-modal="true"
    aria-label="Command palette"
    onkeydown={handleKeydown}
  >
    <input
      class="search"
      type="search"
      bind:value={query}
      {placeholder}
      aria-label="Search commands"
      autofocus
    />
    <ul class="results" role="listbox">
      {#each filtered as item (item.id)}
        <li
          role="option"
          aria-selected="false"
          class="item"
          onclick={() => { onselect?.(item.id); onclose?.(); }}
        >
          <span class="item-label">{item.label}</span>
          {#if item.description}
            <span class="item-desc">{item.description}</span>
          {/if}
        </li>
      {/each}
      {#if filtered.length === 0}
        <li class="empty">No results</li>
      {/if}
    </ul>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.4);
    z-index: 200;
  }

  .palette {
    position: fixed;
    top: 20%;
    left: 50%;
    transform: translateX(-50%);
    width: min(560px, 90vw);
    background: var(--paper);
    border: 1px solid var(--seam);
    border-radius: 12px;
    box-shadow: 0 24px 64px rgba(0,0,0,0.24);
    z-index: 201;
    overflow: hidden;
  }

  .search {
    width: 100%;
    box-sizing: border-box;
    padding: var(--s-4);
    border: none;
    border-bottom: 1px solid var(--rule);
    background: transparent;
    color: var(--ink);
    font: inherit;
    font-size: 16px;
    outline: none;
  }

  .results {
    list-style: none;
    padding: var(--s-2) 0;
    margin: 0;
    max-height: 320px;
    overflow-y: auto;
  }

  .item {
    display: flex;
    align-items: baseline;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-4);
    cursor: pointer;
    transition: background var(--dur-1) var(--ease-out);
  }

  .item:hover { background: var(--ember-soft); }

  .item-label {
    font-size: 14px;
    color: var(--ink);
  }

  .item-desc {
    font-size: 12px;
    color: var(--ink-3);
  }

  .empty {
    padding: var(--s-4);
    color: var(--ink-3);
    font-size: 13px;
    text-align: center;
  }
</style>
