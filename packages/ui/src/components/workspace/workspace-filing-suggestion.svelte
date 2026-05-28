<script lang="ts">
  /**
   * Suggested filing chip — Step 3.4.
   *
   * Non-modal, non-blocking. Appears after the system has a placement heuristic.
   * INVARIANT: system NEVER moves without explicit user confirmation.
   *   "Move here"    → calls onConfirm(targetId, targetPath)
   *   "Choose another" → calls onChoose()
   *   "Ignore"       → calls onIgnore(); chip disappears
   *
   * The chip is presented inline (e.g. below the composer or in the chat header)
   * and does NOT block any user action.
   */

  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";
  import XIcon from "@lucide/svelte/icons/x";
  import * as Button from "../ui/button/index.js";

  type Props = {
    /** Human-readable folder name to suggest, e.g. "Clients / Acme". */
    suggestedPath: string;
    /** The target folder node ID for the move call. */
    targetNodeId: string;
    /** Called when the user clicks "Move here". The caller performs the actual move. */
    onConfirm?: (targetNodeId: string, suggestedPath: string) => void;
    /** Called when the user clicks "Choose another" — open Move-to picker. */
    onChoose?: () => void;
    /** Called when the user ignores / dismisses the suggestion. */
    onIgnore?: () => void;
  };

  let { suggestedPath, targetNodeId, onConfirm, onChoose, onIgnore }: Props = $props();
</script>

<div
  role="status"
  aria-live="polite"
  class="flex items-center gap-2 rounded-lg border border-border/50 bg-muted/40 px-3 py-2 text-sm"
>
  <FolderOpenIcon class="size-4 shrink-0 text-muted-foreground/70" strokeWidth={1.75} aria-hidden="true" />
  <span class="min-w-0 flex-1 truncate text-muted-foreground">
    Suggested: <strong class="font-medium text-foreground">{suggestedPath}</strong>
  </span>
  <div class="flex shrink-0 items-center gap-1">
    <Button.Button
      type="button"
      variant="outline"
      size="sm"
      class="h-6 px-2 text-xs"
      onclick={() => onConfirm?.(targetNodeId, suggestedPath)}
    >
      Move here
    </Button.Button>
    {#if onChoose}
      <Button.Button
        type="button"
        variant="ghost"
        size="sm"
        class="h-6 px-2 text-xs text-muted-foreground hover:text-foreground"
        onclick={onChoose}
      >
        Choose
      </Button.Button>
    {/if}
    <Button.Button
      type="button"
      variant="ghost"
      size="icon-sm"
      class="size-6 text-muted-foreground/60 hover:text-muted-foreground"
      aria-label="Ignore suggestion"
      onclick={onIgnore}
    >
      <XIcon size={12} strokeWidth={2} aria-hidden="true" />
    </Button.Button>
  </div>
</div>
