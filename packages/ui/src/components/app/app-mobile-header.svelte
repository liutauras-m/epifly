<script lang="ts">
  import * as Sidebar from "../ui/sidebar/index.js";
  import type { Snippet } from "svelte";

  type Props = {
    title?: string;
    showSidebarTrigger?: boolean;
    /** Slot for right-side actions in the header. */
    actions?: Snippet;
  };

  let { title = "", showSidebarTrigger = true, actions }: Props = $props();
</script>

<!-- Mobile-only top header. Hidden on md+ where persistent sidebar takes over. -->
<header
  class="flex h-[var(--app-header-height)] items-center gap-2 border-b border-border bg-background px-4 md:hidden"
>
  {#if showSidebarTrigger}
    <Sidebar.Trigger class="-ml-1" />
  {/if}
  {#if title}
    <span class="flex-1 truncate text-sm font-medium {showSidebarTrigger ? '' : 'pl-10'}">{title}</span>
  {:else}
    <span class="flex-1"></span>
  {/if}
  {#if actions}
    {@render actions()}
  {/if}
</header>
