<svelte:options runes={true} />
<script lang="ts">
  /**
   * WorkspaceTree — full-featured workspace file-tree (Phase 4.7).
   *
   * Unified replacement for the old `components/WorkspaceTree` (static stub) and
   * `features/WorkspaceExplorer` (full-featured). Classifies as `features/` because
   * the API uses workspace-domain language (WorkspaceNode, sdk.workspaces.*).
   *
   * Features:
   *   - Expandable folder tree with lazy-loaded children
   *   - Inline search via sdk.workspaces.search (debounced 220ms)
   *   - Create folder / conversation from the toolbar
   *   - Container-query responsive density (--rail-density)
   *
   * Usage:
   *   <WorkspaceTree {sdk} bind:nodes bind:selectedNodeId {onSelectNode} />
   */
  import type { WorkspaceNode } from '@conusai/types';
  import type { ConusSdk } from '@conusai/sdk';

  let {
    sdk,
    nodes = $bindable<WorkspaceNode[]>([]),
    selectedNodeId = $bindable<string | undefined>(),
    onSelectNode,
  }: {
    sdk: ConusSdk;
    nodes?: WorkspaceNode[];
    selectedNodeId?: string;
    onSelectNode?: (node: WorkspaceNode) => void;
  } = $props();

  let expandedFolders = $state(new Set<string>());
  let childNodes = $state(new Map<string, WorkspaceNode[]>());
  let searchQuery = $state('');
  let searchResults = $state<WorkspaceNode[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  // New node form state
  let showNewNodeForm = $state(false);
  let newNodeKind = $state<'folder' | 'conversation'>('folder');
  let newNodeName = $state('');
  let newNodeParentId = $state<string | null>(null);
  let newNodeError = $state('');
  let newNodeBusy = $state(false);

  async function toggleFolder(node: WorkspaceNode) {
    if (expandedFolders.has(node.id)) {
      expandedFolders.delete(node.id);
      expandedFolders = new Set(expandedFolders);
    } else {
      expandedFolders.add(node.id);
      expandedFolders = new Set(expandedFolders);
      if (!childNodes.has(node.id)) {
        const result = await sdk.workspaces.tree(node.id);
        if (!result.error) {
          const updated = new Map(childNodes);
          updated.set(node.id, Array.isArray(result.data) ? result.data : []);
          childNodes = updated;
        }
      }
    }
  }

  function selectNode(node: WorkspaceNode) {
    selectedNodeId = node.id;
    onSelectNode?.(node);
  }

  function openNewNodeForm(parentId: string | null = null) {
    newNodeParentId = parentId;
    newNodeName = '';
    newNodeError = '';
    newNodeKind = 'folder';
    showNewNodeForm = true;
  }

  function closeNewNodeForm() {
    showNewNodeForm = false;
    newNodeName = '';
    newNodeError = '';
  }

  async function submitNewNode(e: SubmitEvent) {
    e.preventDefault();
    let name = newNodeName.trim();
    if (!name) { newNodeError = 'Name is required'; return; }
    if (newNodeKind === 'conversation' && !name.endsWith('.md')) name = `${name}.md`;
    newNodeBusy = true;
    newNodeError = '';
    try {
      const result = await sdk.workspaces.create({ kind: newNodeKind, name, parent_id: newNodeParentId });
      if (result.error) { newNodeError = result.error.message; return; }
      closeNewNodeForm();
      if (newNodeParentId) {
        expandedFolders.add(newNodeParentId);
        expandedFolders = new Set(expandedFolders);
        const childResult = await sdk.workspaces.tree(newNodeParentId);
        if (!childResult.error) {
          const updated = new Map(childNodes);
          updated.set(newNodeParentId, Array.isArray(childResult.data) ? childResult.data : []);
          childNodes = updated;
        }
      } else {
        const treeResult = await sdk.workspaces.tree();
        if (!treeResult.error) nodes = Array.isArray(treeResult.data) ? treeResult.data : [];
      }
    } catch (err) {
      newNodeError = err instanceof Error ? err.message : 'Network error';
    } finally {
      newNodeBusy = false;
    }
  }

  function onSearchInput(e: Event) {
    const q = (e.target as HTMLInputElement).value;
    searchQuery = q;
    if (searchTimer) clearTimeout(searchTimer);
    if (!q.trim()) { searchResults = []; return; }
    searchTimer = setTimeout(async () => {
      const result = await sdk.workspaces.search(q.trim());
      if (!result.error) searchResults = Array.isArray(result.data) ? result.data : [];
      else searchResults = [];
    }, 220);
  }

  function clearSearch() {
    searchQuery = '';
    searchResults = [];
    if (searchTimer) clearTimeout(searchTimer);
  }

  function selectedFolderParent(): string | null {
    if (!selectedNodeId) return null;
    const allNodes = [...nodes, ...[...childNodes.values()].flat()];
    const node = allNodes.find(n => n.id === selectedNodeId);
    return node?.kind === 'folder' ? selectedNodeId : null;
  }
</script>

<section class="workspace-tree" aria-label="Workspace">
  <header class="tree-header">
    <span class="tree-heading label-mono">Workspace</span>
    <button
      type="button" class="icon-btn" aria-label="New folder or conversation"
      onclick={() => openNewNodeForm(selectedFolderParent())}
    >
      <svg viewBox="0 0 18 18" fill="none" stroke="currentColor"
           stroke-width="1.5" stroke-linecap="round" width="16" height="16" aria-hidden="true">
        <line x1="9" y1="3" x2="9" y2="15"/><line x1="3" y1="9" x2="15" y2="9"/>
      </svg>
    </button>
  </header>

  <div class="search-wrap">
    <svg class="search-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor"
         stroke-width="1.5" stroke-linecap="round" width="14" height="14" aria-hidden="true">
      <circle cx="6.5" cy="6.5" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/>
    </svg>
    <input
      class="search-input" type="search" placeholder="Search…"
      autocomplete="off" spellcheck="false" aria-label="Search workspace"
      value={searchQuery} oninput={onSearchInput}
    />
    {#if searchQuery}
      <button class="search-clear" aria-label="Clear search" onclick={clearSearch}>
        <svg viewBox="0 0 12 12" fill="none" stroke="currentColor"
             stroke-width="1.5" stroke-linecap="round" width="10" height="10" aria-hidden="true">
          <line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
        </svg>
      </button>
    {/if}
  </div>

  {#if showNewNodeForm}
    <form class="new-node-form" onsubmit={submitNewNode}>
      <div class="new-node-kind">
        <button type="button" class="kind-btn" class:active={newNodeKind === 'folder'}
          onclick={() => newNodeKind = 'folder'}>📁 Folder</button>
        <button type="button" class="kind-btn" class:active={newNodeKind === 'conversation'}
          onclick={() => newNodeKind = 'conversation'}>📄 Chat</button>
      </div>
      <div class="new-node-row">
        <input
          class="new-node-input" type="text"
          placeholder={newNodeKind === 'folder' ? 'Folder name…' : 'Conversation name…'}
          bind:value={newNodeName} maxlength={80} autocomplete="off"
        />
        <button type="submit" class="new-node-ok" disabled={newNodeBusy} aria-label="Create">
          {newNodeBusy ? '…' : '✓'}
        </button>
        <button type="button" class="new-node-cancel" onclick={closeNewNodeForm} aria-label="Cancel">✕</button>
      </div>
      {#if newNodeError}<div class="new-node-error" role="alert">{newNodeError}</div>{/if}
    </form>
  {/if}

  {#if searchQuery}
    <div class="tree" role="listbox" aria-label="Search results">
      {#if searchResults.length === 0}
        <div class="empty-hint">No matches for "{searchQuery}"</div>
      {:else}
        {#each searchResults as node (node.id)}
          <button
            class="tree-node tree-node-{node.kind}" class:selected={selectedNodeId === node.id}
            role="option" aria-selected={selectedNodeId === node.id}
            onclick={() => selectNode(node)}
          >
            <span class="node-icon" aria-hidden="true">{node.kind === 'folder' ? '📁' : '📄'}</span>
            <span class="node-name">{node.name}</span>
            <span class="node-path">{node.virtual_path}</span>
          </button>
        {/each}
      {/if}
    </div>
  {:else}
    <div class="tree" role="tree" aria-label="Workspace tree">
      {#if nodes.length === 0}
        <div class="empty-hint">No folders yet — click <strong>+</strong> to create one.</div>
      {:else}
        {#each nodes as node (node.id)}
          {@render treeNode(node, 0)}
        {/each}
      {/if}
    </div>
  {/if}
</section>

{#snippet treeNode(node: WorkspaceNode, depth: number)}
  <div class="tree-item" style="--depth:{depth}">
    {#if node.kind === 'folder'}
      <button
        class="tree-node tree-node-folder"
        class:selected={selectedNodeId === node.id}
        class:expanded={expandedFolders.has(node.id)}
        onclick={() => { selectNode(node); toggleFolder(node); }}
        aria-expanded={expandedFolders.has(node.id)}
        aria-selected={selectedNodeId === node.id}
        role="treeitem"
      >
        <span class="node-chevron" aria-hidden="true">{expandedFolders.has(node.id) ? '▾' : '▸'}</span>
        <span class="node-icon" aria-hidden="true">📁</span>
        <span class="node-name">{node.name}</span>
      </button>
      {#if expandedFolders.has(node.id)}
        <div class="tree-children" role="group">
          {#each (childNodes.get(node.id) ?? []) as child (child.id)}
            {@render treeNode(child, depth + 1)}
          {/each}
        </div>
      {/if}
    {:else}
      <button
        class="tree-node tree-node-conversation"
        class:selected={selectedNodeId === node.id}
        onclick={() => selectNode(node)}
        aria-selected={selectedNodeId === node.id}
        role="treeitem"
      >
        <span class="node-icon" aria-hidden="true">📄</span>
        <span class="node-name">{node.name}</span>
      </button>
    {/if}
  </div>
{/snippet}

<style>
  .workspace-tree { display: flex; flex-direction: column; height: 100%; }

  .tree-header {
    display:         flex;
    align-items:     center;
    justify-content: space-between;
    padding:         var(--space-3) var(--space-4);
    border-bottom:   1px solid var(--color-border);
  }
  .tree-heading {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    color:          var(--color-fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .icon-btn {
    display:         flex;
    align-items:     center;
    justify-content: center;
    background:      none;
    border:          none;
    cursor:          pointer;
    color:           var(--color-fg-subtle);
    width:           32px;
    height:          32px;
    border-radius:   var(--radius-xs);
    transition:      background var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .icon-btn:hover { background: var(--color-bg-hover); color: var(--color-fg); }
  .icon-btn:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* ── Search ──────────────────────────────────────────────────────────── */
  .search-wrap {
    position:      relative;
    display:       flex;
    align-items:   center;
    padding:       var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--color-border);
  }
  .search-icon {
    position:       absolute;
    left:           calc(var(--space-3) + 4px);
    color:          var(--color-fg-subtle);
    pointer-events: none;
  }
  .search-input {
    width:          100%;
    padding:        4px var(--space-4) 4px 28px;
    background:     var(--color-bg-hover);
    border:         1px solid var(--color-border);
    border-radius:  var(--radius-xs);
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-meta);
    color:          var(--color-fg);
    outline:        none;
    transition:     border-color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .search-input:focus { border-color: var(--color-accent); }
  .search-clear {
    position:    absolute;
    right:       var(--space-3);
    background:  none;
    border:      none;
    cursor:      pointer;
    color:       var(--color-fg-subtle);
    display:     flex;
    align-items: center;
  }

  /* ── Tree ────────────────────────────────────────────────────────────── */
  .tree        { flex: 1; overflow-y: auto; padding: var(--space-2) 0; }
  .tree-item   { padding-left: calc(var(--depth) * 16px); }
  .tree-node {
    display:       flex;
    align-items:   center;
    gap:           var(--space-2);
    width:         100%;
    padding:       var(--space-1) var(--space-3);
    background:    none;
    border:        none;
    cursor:        pointer;
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-meta);
    color:         var(--color-fg-muted);
    text-align:    left;
    border-radius: var(--radius-xs);
    transition:    background var(--duration-fast) var(--ease-standard);  /* [feedback] */
    min-height:    var(--hit-sm, 36px);
  }
  .tree-node:hover    { background: var(--color-bg-hover); }
  .tree-node.selected { background: var(--color-accent-soft); color: var(--color-fg); }
  .tree-node:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .node-chevron { font-size: 10px; width: 12px; flex-shrink: 0; }
  .node-icon    { flex-shrink: 0; }
  .node-name    { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .node-path    { font-size: var(--font-size-label); color: var(--color-fg-subtle); flex-shrink: 0; }

  .empty-hint {
    padding:   var(--space-3) var(--space-4);
    color:     var(--color-fg-subtle);
    font-size: var(--font-size-meta);
  }

  /* ── New node form ───────────────────────────────────────────────────── */
  .new-node-form   { padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--color-border); }
  .new-node-kind   { display: flex; gap: var(--space-2); margin-bottom: var(--space-2); }
  .kind-btn {
    padding:       2px var(--space-3);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-xs);
    background:    none;
    cursor:        pointer;
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-label);
    color:         var(--color-fg-muted);
    transition:    background var(--duration-fast) var(--ease-standard),  /* [feedback] */
                   border-color var(--duration-fast) var(--ease-standard);
  }
  .kind-btn.active { background: var(--color-accent-soft); border-color: var(--color-accent); color: var(--color-fg); }
  .new-node-row    { display: flex; gap: var(--space-2); }
  .new-node-input {
    flex:          1;
    padding:       4px var(--space-2);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-xs);
    background:    var(--color-bg);
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-meta);
    color:         var(--color-fg);
    outline:       none;
    transition:    border-color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .new-node-input:focus { border-color: var(--color-accent); }
  .new-node-ok, .new-node-cancel {
    padding:       4px var(--space-2);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-xs);
    background:    none;
    cursor:        pointer;
    font-size:     var(--font-size-meta);
    color:         var(--color-fg-muted);
    transition:    background var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .new-node-ok:not(:disabled):hover { background: var(--color-accent-soft); }
  .new-node-cancel:hover             { background: var(--color-bg-hover); }
  .new-node-error {
    color:     var(--color-danger);
    font-size: var(--font-size-label);
    margin-top: var(--space-1);
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-btn, .search-input, .tree-node,
    .kind-btn, .new-node-input, .new-node-ok, .new-node-cancel {
      transition: none;
    }
  }
</style>
