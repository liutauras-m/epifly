<script lang="ts">
  import * as Sidebar from "../ui/sidebar/index.js";
  import type { Snippet } from "svelte";

  type Props = {
    /** Snippet rendered in the sidebar header (e.g. WorkspaceSwitcher). */
    header?: Snippet;
    /** Snippet rendered in the sidebar content area (nav groups). */
    content?: Snippet;
    /** Snippet rendered in the sidebar footer (e.g. AccountMenu). */
    footer?: Snippet;
  };

  let { header, content, footer }: Props = $props();
</script>

<Sidebar.Root collapsible="icon">
  {#if header}
    <Sidebar.Header>
      {@render header()}
    </Sidebar.Header>
  {/if}

  <Sidebar.Content>
    {@render content?.()}
  </Sidebar.Content>

  {#if footer}
    <Sidebar.Footer>
      {@render footer()}
    </Sidebar.Footer>
  {/if}

</Sidebar.Root>
