<svelte:options runes={true} />
<script lang="ts">
  /**
   * ThemeSwitcher — three-way toggle: system / paper / forge (Phase 5.5).
   *
   * Cycles: system → paper → forge → system.
   * 'system' = follow prefers-color-scheme (respects OS dark mode).
   *
   * Lives in gallery toolbar and (per Phase 5.5) AccountMenuButton.
   */
  import { getContext } from 'svelte';
  import type { ThemeStore } from '../stores/themeStore.svelte.js';

  const theme = getContext<ThemeStore>('conusai.theme');

  const labels: Record<string, string> = {
    system: 'System theme (follows OS)',
    paper:  'Switch to dark theme (Forge)',
    forge:  'Switch to system theme',
  };
  const titles: Record<string, string> = {
    system: 'System',
    paper:  'Paper (light)',
    forge:  'Forge (dark)',
  };
</script>

<button
  class="theme-switcher theme-{theme.preference ?? 'system'}"
  onclick={() => theme.toggle()}
  aria-label={labels[theme.preference ?? 'system']}
  title={titles[theme.preference ?? 'system']}
>
  {#if (theme.preference ?? 'system') === 'system'}
    <!-- System / auto icon -->
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <circle cx="10" cy="10" r="8"/>
      <path d="M10 2v16M2 10h16" stroke-dasharray="2 2"/>
      <circle cx="10" cy="10" r="3" fill="currentColor" stroke="none"/>
    </svg>
  {:else if theme.preference === 'paper'}
    <!-- Sun / light icon -->
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <circle cx="10" cy="10" r="4"/>
      <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.93 4.93l1.41 1.41M13.66 13.66l1.41 1.41M4.93 15.07l1.41-1.41M13.66 6.34l1.41-1.41"/>
    </svg>
  {:else}
    <!-- Moon / dark icon -->
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
      <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z"/>
    </svg>
  {/if}
</button>

<style>
  .theme-switcher {
    display:         flex;
    align-items:     center;
    justify-content: center;
    width:           32px;
    height:          32px;
    border:          1px solid var(--color-border);
    border-radius:   var(--radius-sm);
    background:      transparent;
    color:           var(--color-fg-subtle);
    cursor:          pointer;
    outline:         none;
    transition:
      color        var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard),
      background   var(--duration-fast) var(--ease-standard);
  }

  .theme-switcher:hover {
    color:        var(--color-fg);
    border-color: var(--color-border-strong);
  }

  .theme-switcher:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* Active state per preference */
  .theme-system { border-color: var(--color-border-strong); }
  .theme-paper  { color: var(--color-fg-muted); }
  .theme-forge  { color: var(--color-fg-muted); }

  .theme-switcher svg {
    width:  16px;
    height: 16px;
  }
</style>
