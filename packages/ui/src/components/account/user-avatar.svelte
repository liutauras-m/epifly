<script lang="ts">
  import * as Avatar from "../ui/avatar/index.js";
  import { cn } from "../../utils/cn.js";

  type Props = {
    name?: string;
    email?: string;
    avatarUrl?: string;
    class?: string;
  };

  let { name, email, avatarUrl, class: className }: Props = $props();

  let initials = $derived(
    name
      ? name
          .split(" ")
          .slice(0, 2)
          .map((s) => s[0]?.toUpperCase() ?? "")
          .join("")
      : "?"
  );
</script>

<Avatar.Avatar class={cn("h-8 w-8", className)}>
  {#if avatarUrl}
    <Avatar.AvatarImage src={avatarUrl} alt={name ?? "User avatar"} />
  {/if}
  <Avatar.AvatarFallback
    class="bg-primary text-xs font-medium text-primary-foreground"
  >
    {initials}
  </Avatar.AvatarFallback>
</Avatar.Avatar>
