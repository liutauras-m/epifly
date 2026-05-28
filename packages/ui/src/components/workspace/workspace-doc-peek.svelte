<script lang="ts">
  /**
   * WorkspaceDocPeek — read-only "View as document" panel.
   *
   * Slides in from the right when a thread/node's projected Markdown is requested.
   * Primary click on thread rows still opens chat; this is secondary affordance.
   *
   * Phase 4.1: fetch + show content.
   * Phase 4.3: summary line at top when present.
   * Phase 8.2: related items list.
   */

  import XIcon from "@lucide/svelte/icons/x";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import * as Button from "../ui/button/index.js";
  import * as Skeleton from "../ui/skeleton/index.js";
  import { cn } from "../../utils/cn.js";

  type RelatedItem = {
    id: string;
    name: string;
    kind: "folder" | "thread" | "document";
  };

  type Props = {
    open?: boolean;
    nodeName?: string | null;
    summary?: string | null;
    content?: string | null;
    isLoading?: boolean;
    error?: string | null;
    /** Phase 8.2 — related nodes (relatedNodeIds + linkedFileIds resolved to names) */
    relatedItems?: RelatedItem[];
    onClose?: () => void;
    /** Navigate to a related item (by id). */
    onNavigateRelated?: (id: string) => void;
    class?: string;
  };

  let {
    open = false,
    nodeName = null,
    summary = null,
    content = null,
    isLoading = false,
    error = null,
    relatedItems = [],
    onClose,
    onNavigateRelated,
    class: className,
  }: Props = $props();

  let dialogEl = $state<HTMLDivElement | null>(null);

  // Focus trap: when peek opens, move focus to the dialog.
  $effect(() => {
    if (open && dialogEl) {
      dialogEl.focus();
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose?.();
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <div
    class="fixed inset-0 z-40 bg-black/20 backdrop-blur-[1px]"
    aria-hidden="true"
    onclick={onClose}
  ></div>

  <!-- Peek panel -->
  <div
    bind:this={dialogEl}
    role="dialog"
    aria-modal="true"
    aria-label={nodeName ? `Document: ${nodeName}` : "Document"}
    tabindex="-1"
    class={cn(
      "fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col bg-background shadow-xl outline-none",
      "border-l border-border/60",
      "animate-in slide-in-from-right duration-[var(--motion-standard)] ease-[var(--ease-standard)]",
      className
    )}
    onkeydown={handleKeydown}
  >
    <!-- Header -->
    <div class="flex shrink-0 items-center gap-2 border-b border-border/50 px-4 py-3">
      <FileTextIcon class="size-4 shrink-0 text-muted-foreground/60" strokeWidth={1.75} aria-hidden="true" />
      <span class="flex-1 truncate text-sm font-medium text-foreground/90">
        {nodeName ?? "Document"}
      </span>
      <Button.Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="size-7 shrink-0 text-muted-foreground/60 hover:text-foreground"
        aria-label="Close document view"
        onclick={onClose}
      >
        <XIcon size={14} strokeWidth={2} aria-hidden="true" />
      </Button.Button>
    </div>

    <!-- Summary chip (Phase 4.3) -->
    {#if summary}
      <div class="shrink-0 border-b border-border/30 bg-muted/30 px-4 py-2">
        <p class="text-xs text-muted-foreground/80 line-clamp-2">{summary}</p>
      </div>
    {/if}

    <!-- Body -->
    <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-5 py-4">
      {#if isLoading}
        <div class="space-y-3" aria-label="Loading document">
          <Skeleton.Skeleton class="h-4 w-3/4 rounded-md" />
          <Skeleton.Skeleton class="h-4 w-full rounded-md" />
          <Skeleton.Skeleton class="h-4 w-5/6 rounded-md" />
          <Skeleton.Skeleton class="mt-4 h-4 w-2/3 rounded-md" />
          <Skeleton.Skeleton class="h-4 w-full rounded-md" />
        </div>
      {:else if error}
        <p role="alert" class="text-sm text-destructive">{error}</p>
      {:else if content}
        <!-- Rendered as pre-formatted text — full Markdown renderer can be swapped in later. -->
        <pre class="whitespace-pre-wrap font-sans text-sm leading-relaxed text-foreground/85">{content}</pre>
      {:else}
        <p class="text-sm text-muted-foreground/50">No content yet.</p>
      {/if}

      <!-- Phase 8.2 — Related items -->
      {#if relatedItems.length > 0}
        <div class="mt-6 border-t border-border/40 pt-4">
          <p class="mb-2 text-[0.68rem] font-medium uppercase tracking-wide text-muted-foreground/50">
            Related
          </p>
          <ul class="flex flex-col gap-1">
            {#each relatedItems as item (item.id)}
              <li>
                <button
                  type="button"
                  class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-foreground/75 hover:bg-muted/60 hover:text-foreground"
                  onclick={() => onNavigateRelated?.(item.id)}
                >
                  <FileTextIcon class="size-3.5 shrink-0 text-muted-foreground/50" strokeWidth={1.75} aria-hidden="true" />
                  <span class="flex-1 truncate text-left">{item.name}</span>
                </button>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>
  </div>
{/if}
