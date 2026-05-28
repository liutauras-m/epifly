<script lang="ts">
  import AiToolProgress, { type AiToolProgressState } from "../app/ai-tool-progress.svelte";
  import { cn } from "../../utils/cn.js";

  type Status =
    | "idle"
    | "starting"
    | "thinking"
    | "writing"
    | "using_tool"
    | "waiting"
    | "finished"
    | "stopped"
    | "streaming"
    | "error";

  type Props = {
    status: Status;
    class?: string;
  };

  const statusText: Record<Status, string | null> = {
    idle: null,
    starting: "Starting",
    thinking: "Thinking",
    writing: "Writing",
    using_tool: "Using a tool",
    waiting: "Waiting for results",
    finished: "Finished",
    stopped: "Stopped",
    streaming: "Writing",
    error: "Something went wrong. Please try again."
  };

  const statusDetail: Record<Status, string | null> = {
    idle: null,
    starting: "Preparing the next step",
    thinking: "Reading the conversation",
    writing: "Composing the response",
    using_tool: "Checking the workspace",
    waiting: "Waiting for the tool response",
    finished: "Response complete",
    stopped: "Generation stopped",
    streaming: "Composing the response",
    error: "The response paused before it finished"
  };

  const statusState: Record<Status, AiToolProgressState | null> = {
    idle: null,
    starting: "starting",
    thinking: "thinking",
    writing: "writing",
    using_tool: "working",
    waiting: "waiting",
    finished: "finished",
    stopped: "stopped",
    streaming: "writing",
    error: "error"
  };

  let { status, class: className }: Props = $props();
  let text = $derived(statusText[status]);
  let detail = $derived(statusDetail[status]);
  let state = $derived(statusState[status]);
</script>

{#if text && state}
  <div
    class={cn(
      "mx-auto flex w-full max-w-3xl items-center px-6 py-1.5 text-xs",
      status === "error" ? "text-destructive" : "text-muted-foreground",
      className
    )}
    role={status === "error" ? "alert" : "status"}
    aria-live={status === "error" ? "assertive" : "polite"}
  >
    <AiToolProgress
      {state}
      variant="pill"
      size="sm"
      label={text}
      detail={detail ?? undefined}
      showLabel={true}
    />
  </div>
{/if}
