<script lang="ts">
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

  let { kind, label, class: className }: Props = $props();
</script>

<div
  class={cn(
    "flex items-center gap-2 py-1 text-xs text-muted-foreground",
    className
  )}
>
  <span
    class="h-1.5 w-1.5 rounded-full bg-muted-foreground/50"
    aria-hidden="true"
  ></span>
  <span>{label ?? kindLabel[kind]}</span>
</div>
