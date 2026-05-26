<script lang="ts">
  import CircleIcon from "@lucide/svelte/icons/circle";
  import * as ScrollArea from "../ui/scroll-area/index.js";
</script>

<section class="flex min-h-0 w-full flex-col bg-background" aria-label="Jobs">
  <header class="flex h-14 shrink-0 items-center justify-between px-4">
    <h2 class="text-lg font-semibold tracking-[-0.03em] text-foreground">Jobs</h2>
    <CircleIcon size={16} strokeWidth={1.75} class="text-muted-foreground" aria-hidden="true" />
  </header>
  <ScrollArea.ScrollArea class="min-h-0 flex-1">
    <div class="p-4">
      <p class="text-sm text-muted-foreground">No active jobs</p>
    </div>
  </ScrollArea.ScrollArea>
</section>