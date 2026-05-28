<script lang="ts">
  import AiToolProgress, { type AiToolProgressState } from "../app/ai-tool-progress.svelte";
  import { cn } from "../../utils/cn.js";

  type EventKind = "tool_start" | "tool_result" | "routing_meta" | "resource_invalidated";

  type Props = {
    kind: EventKind;
    label?: string;
    class?: string;
  };

  const kindLabel: Record<EventKind, string> = {
    tool_start: "Running tool",
    tool_result: "Tool result",
    routing_meta: "Routing",
    resource_invalidated: "Resource updated"
  };

  const kindState: Record<EventKind, AiToolProgressState> = {
    tool_start: "working",
    tool_result: "stopped",
    routing_meta: "thinking",
    resource_invalidated: "stopped"
  };

  let { kind, label, class: className }: Props = $props();
</script>

<div
  class={cn(
    "flex items-center gap-2 py-1 text-xs text-muted-foreground",
    className
  )}
>
  <AiToolProgress state={kindState[kind]} size="sm" showLabel={false} />
  <span>{label ?? kindLabel[kind]}</span>
</div>
