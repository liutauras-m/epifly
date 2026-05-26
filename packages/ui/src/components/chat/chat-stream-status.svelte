<script lang="ts">
  import { cn } from "../../utils/cn.js";

  type Status = "idle" | "streaming" | "thinking" | "error";

  type Props = {
    status: Status;
    class?: string;
  };

  const statusText: Record<Status, string | null> = {
    idle: null,
    streaming: "Generating…",
    thinking: "Thinking…",
    error: "Something went wrong. Please try again."
  };

  let { status, class: className }: Props = $props();
  let text = $derived(statusText[status]);
</script>

{#if text}
  <div
    class={cn(
      "flex items-center gap-2 px-4 py-2 text-xs",
      status === "error" ? "text-destructive" : "text-muted-foreground",
      className
    )}
    role={status === "error" ? "alert" : "status"}
    aria-live={status === "error" ? "assertive" : "polite"}
  >
    {#if status !== "error"}
      <span
        class="h-1.5 w-1.5 animate-pulse rounded-full bg-current"
        aria-hidden="true"
      ></span>
    {/if}
    {text}
  </div>
{/if}
