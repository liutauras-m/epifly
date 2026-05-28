<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import * as Avatar from "../ui/avatar/index.js";

  type Role = "user" | "assistant";

  type Props = {
    role: Role;
    content: string;
    pending?: boolean;
    class?: string;
  };

  let { role, content, pending = false, class: className }: Props = $props();
</script>

<div
  class={cn(
    "app-chat-message flex gap-3",
    role === "assistant" && pending && !content && "message ai thinking",
    role === "user" ? "flex-row-reverse" : "flex-row",
    className
  )}
>
  <Avatar.Avatar size="sm" class="mt-0.5">
    <Avatar.AvatarFallback
      class={cn(
        "text-[0.68rem] font-medium",
        role === "user"
          ? "bg-primary text-primary-foreground"
          : "bg-muted text-muted-foreground"
      )}
    >
      {role === "user" ? "U" : "AI"}
    </Avatar.AvatarFallback>
  </Avatar.Avatar>

  <!-- Bubble -->
  <div
    class={cn(
      "app-chat-bubble max-w-[80%] rounded-[18px] px-4 py-2.5 text-sm leading-relaxed shadow-sm",
      role === "user"
        ? "rounded-tr-[14px] bg-foreground text-background"
        : "rounded-tl-[14px] border border-border/60 bg-background/90 text-foreground"
    )}
  >
    {#if content}
      <!-- Render content; in a real app, pass through a markdown renderer here -->
      <p class="whitespace-pre-wrap break-words">{content}</p>
    {/if}
  </div>
</div>
