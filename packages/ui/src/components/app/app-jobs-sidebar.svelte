<script lang="ts">
  import * as ScrollArea from "../ui/scroll-area/index.js";
</script>

<section class="flex h-full min-h-0 w-full flex-col bg-background" aria-label="Jobs">
  <!-- pr-12 leaves room for the sheet's close button on mobile (overlaid top-right). -->
  <header class="flex h-14 shrink-0 items-center border-b border-border px-4 pr-12">
    <h2 class="truncate text-lg font-semibold tracking-[-0.03em] text-foreground">Jobs</h2>
  </header>
  <ScrollArea.ScrollArea class="min-h-0 flex-1">
    <div class="p-4">
      <p class="text-sm text-muted-foreground">No active jobs</p>
    </div>
  </ScrollArea.ScrollArea>
</section>