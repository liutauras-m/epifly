<script lang="ts">
  import { page } from "$app/state";
  import { AppJobsSidebar, AppMain, AppNavigationSidebar, AppShell } from "@epifly/ui";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();
</script>

{#snippet sidebar()}
  <AppNavigationSidebar activePath={page.url.pathname} />
{/snippet}

{#snippet rightSidebar()}
  <AppJobsSidebar />
{/snippet}

<AppShell {sidebar} {rightSidebar}>
  <AppMain class="bg-background">
    {@render children?.()}
  </AppMain>
</AppShell>
