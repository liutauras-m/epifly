<script lang="ts">
  export interface Tab {
    id: string;
    label: string;
    url?: string;
  }

  interface Props {
    tabs: Tab[];
    activeId?: string;
    onselect?: (id: string) => void;
    onclose?: (id: string) => void;
    oncreate?: () => void;
  }

  let { tabs, activeId, onselect, onclose, oncreate }: Props = $props();
</script>

<div class="tab-strip" role="tablist">
  {#each tabs as tab (tab.id)}
    <button
      role="tab"
      class="tab"
      class:active={tab.id === activeId}
      aria-selected={tab.id === activeId}
      onclick={() => onselect?.(tab.id)}
    >
      <span class="tab-label">{tab.label}</span>
      <span
        class="tab-close"
        role="button"
        tabindex="0"
        aria-label="Close tab {tab.label}"
        onclick={(e) => { e.stopPropagation(); onclose?.(tab.id); }}
        onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); onclose?.(tab.id); } }}
      >×</span>
    </button>
  {/each}
  <button class="tab-new" aria-label="New tab" onclick={() => oncreate?.()}>+</button>
</div>

<style>
  .tab-strip {
    display: flex;
    align-items: center;
    background: var(--paper-2);
    border-bottom: 1px solid var(--rule);
    overflow-x: auto;
    scrollbar-width: none;
    min-height: 36px;
  }

  .tab-strip::-webkit-scrollbar { display: none; }

  .tab {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    padding: 0 var(--s-3);
    height: 36px;
    border: none;
    border-right: 1px solid var(--rule);
    background: transparent;
    color: var(--ink-2);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
    max-width: 180px;
    transition: background var(--dur-1) var(--ease-out);
  }

  .tab:hover { background: var(--paper-3); }

  .tab.active {
    background: var(--paper);
    color: var(--ink);
    border-bottom: 2px solid var(--ember);
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tab-close {
    flex-shrink: 0;
    border: none;
    background: transparent;
    color: var(--ink-3);
    cursor: pointer;
    padding: 0 2px;
    font-size: 16px;
    line-height: 1;
  }

  .tab-close:hover { color: var(--danger); }

  .tab-new {
    border: none;
    background: transparent;
    color: var(--ink-3);
    cursor: pointer;
    padding: 0 var(--s-3);
    font-size: 18px;
    height: 36px;
  }

  .tab-new:hover { color: var(--ember); }
</style>
