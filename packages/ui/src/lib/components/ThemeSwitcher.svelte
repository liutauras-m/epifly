<script lang="ts">
  import { getContext } from 'svelte';
  import type { ThemeStore } from '../stores/themeStore.js';

  const theme = getContext<ThemeStore>('conusai.theme');
</script>

<button
  class="theme-switcher"
  onclick={() => theme.toggle()}
  aria-label={theme.current === 'paper' ? 'Switch to dark theme' : 'Switch to light theme'}
  title={theme.current === 'paper' ? 'Forge (dark)' : 'Paper (light)'}
>
  {#if theme.current === 'paper'}
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <circle cx="10" cy="10" r="4"/>
      <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.93 4.93l1.41 1.41M13.66 13.66l1.41 1.41M4.93 15.07l1.41-1.41M13.66 6.34l1.41-1.41"/>
    </svg>
  {:else}
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z"/>
    </svg>
  {/if}
</button>

<style>
  .theme-switcher {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--ink-3);
    cursor: pointer;
    transition: color var(--dur-1), border-color var(--dur-1);
  }
  .theme-switcher:hover {
    color: var(--ink);
    border-color: var(--seam);
  }
  .theme-switcher svg {
    width: 16px;
    height: 16px;
  }
</style>
