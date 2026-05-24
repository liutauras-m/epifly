<svelte:options runes={true} />
<script lang="ts">
  /**
   * ArtifactsScreen — Phase 4.8
   * Responsive grid of ArtifactRow cards (3-col desktop / 2-col tablet / 1-col mobile).
   * Preview opens in <Sheet> on mobile, side panel on ≥768 px.
   */
  import type { ConusSdk } from '@conusai/sdk';
  import ArtifactRow from './ArtifactRow.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Sheet from '../../components/Sheet.svelte';

  let { sdk }: { sdk: ConusSdk } = $props();

  type Artifact = {
    id: string;
    name: string;
    kind: string;
    size_bytes?: number;
    updated_at?: string;
  };

  let artifacts = $state<Artifact[]>([]);
  let loading = $state(true);
  let selected = $state<Artifact | null>(null);
  let previewOpen = $state(false);

  $effect(() => { loadArtifacts(); });

  async function loadArtifacts() {
    loading = true;
    const res = await sdk.workspaces.tree();
    if (!res.error && res.data) {
      const raw = res.data as any;
      const all: Artifact[] = Array.isArray(raw) ? raw : (raw?.nodes ?? []);
      artifacts = all.filter((n) => n.kind === 'file');
    }
    loading = false;
  }

  function openPreview(a: Artifact) {
    selected = a;
    previewOpen = true;
  }

  function closePreview() {
    previewOpen = false;
  }

  function fmtSize(n: number) {
    if (n < 1024) return `${n} B`;
    if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1048576).toFixed(1)} MB`;
  }

  function fmtDate(ts: string) {
    return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  }
</script>

<div class="artifacts-screen">
  <div class="artifacts-main">
    {#if loading}
      <div class="artifacts-grid" role="list" aria-label="Loading artifacts">
        {#each [1, 2, 3, 4, 5, 6] as _}
          <div class="skeleton-card" role="presentation"></div>
        {/each}
      </div>
    {:else if artifacts.length === 0}
      <EmptyState
        kind="no-artifacts"
        title="No artifacts yet"
        body="Files attached to conversations will appear here."
      />
    {:else}
      <div class="artifacts-grid">
        {#each artifacts as a (a.id)}
          <ArtifactRow
            name={a.name}
            size={a.size_bytes}
            updatedAt={a.updated_at}
            selected={selected?.id === a.id}
            onClick={() => openPreview(a)}
          />
        {/each}
      </div>
    {/if}
  </div>

  <!-- Desktop side panel — visible ≥768 px when an artifact is selected -->
  {#if selected}
    <aside class="preview-panel" aria-label="Artifact preview">
      <header class="preview-header">
        <span class="preview-title">{selected.name}</span>
        <button class="preview-close" onclick={closePreview} aria-label="Close preview">
          <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12" aria-hidden="true">
            <line x1="1" y1="1" x2="11" y2="11"/>
            <line x1="11" y1="1" x2="1" y2="11"/>
          </svg>
        </button>
      </header>
      <div class="preview-body">
        <dl class="preview-meta">
          {#if selected.size_bytes != null}
            <dt>Size</dt>
            <dd>{fmtSize(selected.size_bytes)}</dd>
          {/if}
          {#if selected.updated_at}
            <dt>Modified</dt>
            <dd>{fmtDate(selected.updated_at)}</dd>
          {/if}
          <dt>Type</dt>
          <dd>{selected.kind}</dd>
        </dl>
      </div>
    </aside>
  {/if}
</div>

<!-- Mobile Sheet preview — hidden ≥768 px via CSS -->
<Sheet open={previewOpen} onclose={closePreview} label={selected?.name ?? 'Artifact'}>
  {#snippet children()}
    {#if selected}
      <div class="sheet-preview">
        <dl class="preview-meta">
          {#if selected.size_bytes != null}
            <dt>Size</dt>
            <dd>{fmtSize(selected.size_bytes)}</dd>
          {/if}
          {#if selected.updated_at}
            <dt>Modified</dt>
            <dd>{fmtDate(selected.updated_at)}</dd>
          {/if}
          <dt>Type</dt>
          <dd>{selected.kind}</dd>
        </dl>
      </div>
    {/if}
  {/snippet}
</Sheet>

<style>
  .artifacts-screen {
    display: flex;
    flex: 1;
    overflow: hidden;
    background: var(--color-bg);
  }

  .artifacts-main {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4);
  }

  /* Responsive grid: 1→2→3 columns via breakpoints */
  .artifacts-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--space-3);
  }

  @media (min-width: 480px) {
    .artifacts-grid { grid-template-columns: repeat(2, 1fr); }
  }

  @media (min-width: 1024px) {
    .artifacts-grid { grid-template-columns: repeat(3, 1fr); }
  }

  /* Desktop side panel */
  .preview-panel {
    display: none;
    width: 280px;
    flex-shrink: 0;
    border-left: 1px solid var(--color-border);
    background: var(--color-bg-raised);
    flex-direction: column;
  }

  @media (min-width: 768px) {
    .preview-panel { display: flex; }
    /* Hide the Sheet on desktop; side panel handles it */
  }

  .preview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  .preview-title {
    font-size: var(--font-size-body);
    font-weight: 500;
    color: var(--color-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    cursor: pointer;
    color: var(--color-fg-subtle);
    flex-shrink: 0;
  }

  .preview-close:hover { background: var(--color-bg-hover); }

  .preview-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4);
  }

  .preview-meta {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-1) var(--space-4);
    margin: 0;
  }

  .preview-meta dt {
    font-size: var(--font-size-label);
    color: var(--color-fg-subtle);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .preview-meta dd {
    font-size: var(--font-size-body);
    color: var(--color-fg);
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sheet-preview { padding: var(--space-4) 0; }

  /* Skeleton loading cards */
  .skeleton-card {
    height: 80px;
    background: var(--color-bg-raised);
    border-radius: var(--radius-md);
    animation: shimmer 1.2s ease-in-out infinite;  /* [feedback] loading in progress */
  }

  @keyframes shimmer { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }

  @media (prefers-reduced-motion: reduce) {
    .skeleton-card { animation: none; }
  }
</style>
