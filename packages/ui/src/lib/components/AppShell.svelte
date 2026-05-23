<script lang="ts">
  import "../tokens.css";

  interface Props {
    title?: string;
    children: import("svelte").Snippet;
    sidebar?: import("svelte").Snippet;
  }

  let { title = "ConusAI", children, sidebar }: Props = $props();
</script>

<svelte:head>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
</svelte:head>

<div class="shell">
  {#if sidebar}
    <aside class="sidebar">
      {@render sidebar()}
    </aside>
  {/if}
  <main class="main">
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: flex;
    height: 100dvh;
    background: var(--paper);
    color: var(--ink);
    font-family: var(--font-family-sans);
  }

  .sidebar {
    width: 260px;
    flex-shrink: 0;
    background: var(--paper-2);
    border-right: 1px solid var(--rule);
    overflow-y: auto;
  }

  .main {
    flex: 1;
    overflow-y: auto;
    min-width: 0;
  }
</style>
