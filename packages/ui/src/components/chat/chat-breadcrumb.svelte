<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import MapPinIcon from "@lucide/svelte/icons/map-pin";
  import { cn } from "../../utils/cn.js";

  type Props = {
    /**
     * Dot-separated or slash-separated virtual path, e.g. "Clients/Acme/Kickoff"
     * or "". When empty or absent the component renders nothing.
     */
    virtualPath?: string | null;
    /** Called when a crumb is clicked. Receives the partial path up to that segment. */
    onCrumbClick?: (partialPath: string) => void;
    class?: string;
  };

  let { virtualPath = null, onCrumbClick, class: className }: Props = $props();

  /** Split path into segments, filter empty strings. */
  const segments = $derived(
    virtualPath
      ? virtualPath
          .split("/")
          .map((s) => s.trim())
          .filter((s) => s.length > 0)
      : []
  );

  /** Build the partial path for crumb[i]: first i+1 segments joined with "/". */
  function partialPath(index: number): string {
    return segments.slice(0, index + 1).join("/");
  }
</script>

{#if segments.length > 0}
  <nav
    class={cn("flex min-w-0 items-center gap-0.5 text-[0.7rem] text-muted-foreground/70", className)}
    aria-label="Workspace location"
  >
    <MapPinIcon class="mr-0.5 size-3 shrink-0 text-muted-foreground/50" strokeWidth={1.75} aria-hidden="true" />
    {#each segments as segment, i (i)}
      {#if i > 0}
        <ChevronRightIcon class="size-2.5 shrink-0 text-muted-foreground/35" strokeWidth={2} aria-hidden="true" />
      {/if}
      {#if onCrumbClick}
        <button
          type="button"
          class="max-w-[10rem] truncate rounded px-0.5 py-px outline-none transition-colors hover:text-foreground/80 focus-visible:ring-1 focus-visible:ring-ring/40"
          onclick={() => onCrumbClick(partialPath(i))}
          aria-label="Go to {partialPath(i)}"
        >
          {segment}
        </button>
      {:else}
        <span class="max-w-[10rem] truncate">{segment}</span>
      {/if}
    {/each}
  </nav>
{/if}
