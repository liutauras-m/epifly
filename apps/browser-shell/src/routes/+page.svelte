<script lang="ts">
  import { WorkspaceTree, ArtifactPreview, CapabilityCard } from "@conusai/ui";
  import type { WorkspaceNode, CapabilityCard as CardType } from "@conusai/types";

  let nodes = $state<WorkspaceNode[]>([]);
  let selectedNode = $state<WorkspaceNode | null>(null);
  let artifactContent = $state<string | null>(null);
</script>

<div class="home">
  <aside class="workspace-panel">
    <h2 class="panel-title">Workspace</h2>
    <WorkspaceTree
      {nodes}
      selectedId={selectedNode?.id}
      onselect={(n) => {
        selectedNode = n;
        artifactContent = null;
      }}
    />
  </aside>

  <main class="artifact-panel">
    <ArtifactPreview content={artifactContent} mimeType="application/json" />
  </main>
</div>

<style>
  .home {
    display: flex;
    height: 100%;
  }

  .workspace-panel {
    width: 240px;
    flex-shrink: 0;
    border-right: 1px solid var(--rule);
    padding: var(--s-3);
    overflow-y: auto;
  }

  .panel-title {
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-3);
    margin: 0 0 var(--s-3);
  }

  .artifact-panel {
    flex: 1;
    padding: var(--s-4);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
