<script lang="ts">
  interface Props {
    content: string | null;
    mimeType?: string;
    loading?: boolean;
  }

  let { content, mimeType = "text/plain", loading = false }: Props = $props();

  let isJson = $derived(mimeType.includes("json"));
  let formatted = $derived(() => {
    if (!content || !isJson) return content;
    try {
      return JSON.stringify(JSON.parse(content), null, 2);
    } catch {
      return content;
    }
  });
</script>

<div class="artifact-preview" aria-label="Artifact preview">
  {#if loading}
    <div class="loading" aria-label="Loading…">
      <span class="spinner" aria-hidden="true"></span>
    </div>
  {:else if content}
    <pre class="content" tabindex="0">{formatted()}</pre>
  {:else}
    <div class="empty">No artifact selected</div>
  {/if}
</div>

<style>
  .artifact-preview {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: 8px;
    overflow: hidden;
  }

  .content {
    margin: 0;
    padding: var(--s-4);
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--ink-2);
    flex: 1;
    outline: none;
  }

  .content:focus-visible {
    outline: 2px solid var(--ember);
    outline-offset: -2px;
  }

  .loading, .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--ink-3);
    font-size: 13px;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--rule);
    border-top-color: var(--ember);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
