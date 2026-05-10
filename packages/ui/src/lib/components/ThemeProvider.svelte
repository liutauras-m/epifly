<script lang="ts">
  import { setContext } from 'svelte';
  import { createThemeStore } from '../stores/themeStore.js';
  import type { ThemeAdapter } from '../stores/themeStore.js';

  let {
    adapter = undefined,
    children,
    onThemeChange,
  }: {
    adapter?: ThemeAdapter;
    children: import('svelte').Snippet;
    onThemeChange?: (theme: string) => void;
  } = $props();

  const theme = createThemeStore(adapter);
  setContext('conusai.theme', theme);

  $effect(() => {
    onThemeChange?.(theme.current);
  });
</script>

{@render children()}
