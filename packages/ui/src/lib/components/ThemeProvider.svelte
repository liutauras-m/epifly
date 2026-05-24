<script lang="ts">
  import { setContext, untrack } from 'svelte';
  import { createThemeStore } from '../stores/themeStore.svelte.js';
  import type { ThemeAdapter } from '../stores/themeStore.svelte.js';

  let {
    adapter = undefined,
    children,
    onThemeChange,
  }: {
    adapter?: ThemeAdapter;
    children: import('svelte').Snippet;
    onThemeChange?: (theme: string) => void;
  } = $props();

  // adapter is an initialization-only prop — untrack() signals intentional static capture.
  const theme = createThemeStore(untrack(() => adapter));
  setContext('conusai.theme', theme);

  $effect(() => {
    onThemeChange?.(theme.current);
  });
</script>

{@render children()}
