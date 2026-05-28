<script lang="ts">
  /**
   * Command palette — Step 3.5.
   *
   * Global Cmd/Ctrl+K opens this overlay. Commands are passed in as an array.
   * The palette does NOT implement the commands — it fires callbacks so the
   * feature code can execute them. This keeps @epifly/ui SDK-free.
   *
   * Mounted in both app layouts. Close on Escape or backdrop click.
   */

  import SearchIcon from "@lucide/svelte/icons/search";
  import { cn } from "../../utils/cn.js";

  export type PaletteCommand = {
    id: string;
    label: string;
    /** Short keyboard hint displayed on the right (e.g. "⌘K", "⌘N"). */
    shortcut?: string;
    /** Icon component — rendered as a Svelte snippet. */
    group?: string;
    onRun: () => void;
  };

  type Props = {
    open?: boolean;
    commands?: PaletteCommand[];
    onClose?: () => void;
  };

  let { open = false, commands = [], onClose }: Props = $props();

  let query = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);
  let selectedIndex = $state(0);

  const filtered = $derived(
    query.trim()
      ? commands.filter((c) =>
          c.label.toLowerCase().includes(query.trim().toLowerCase())
        )
      : commands
  );

  $effect(() => {
    if (open) {
      query = "";
      selectedIndex = 0;
      requestAnimationFrame(() => inputEl?.focus());
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose?.();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      return;
    }
    if (event.key === "Enter" && filtered[selectedIndex]) {
      event.preventDefault();
      filtered[selectedIndex].onRun();
      onClose?.();
    }
  }

  $effect(() => {
    // Reset index when filtered list changes
    selectedIndex = 0;
  });
</script>

{#if open}
  <!-- Backdrop -->
  <div
    class="fixed inset-0 z-50 bg-black/40 backdrop-blur-[2px]"
    role="button"
    aria-label="Close command palette"
    tabindex="-1"
    onclick={onClose}
    onkeydown={(e) => { if (e.key === "Escape") onClose?.(); }}
  ></div>

  <!-- Palette -->
  <div
    role="dialog"
    aria-label="Command palette"
    aria-modal="true"
    tabindex="-1"
    class="fixed left-1/2 top-[20%] z-50 w-full max-w-lg -translate-x-1/2 rounded-xl border border-border bg-popover shadow-2xl"
    onkeydown={handleKeydown}
  >
    <!-- Search input -->
    <div class="flex items-center gap-2 border-b border-border px-4 py-3">
      <SearchIcon class="size-4 shrink-0 text-muted-foreground/60" strokeWidth={1.75} aria-hidden="true" />
      <input
        bind:this={inputEl}
        bind:value={query}
        type="text"
        placeholder="Type a command…"
        class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/50"
        aria-label="Command search"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-listbox"
        aria-activedescendant={filtered[selectedIndex] ? `cmd-${filtered[selectedIndex].id}` : undefined}
      />
      <kbd class="rounded border border-border bg-muted px-1.5 py-0.5 text-[0.6rem] text-muted-foreground/70">Esc</kbd>
    </div>

    <!-- Command list -->
    <ul
      id="palette-listbox"
      role="listbox"
      aria-label="Commands"
      class="max-h-72 overflow-y-auto py-2"
    >
      {#if filtered.length === 0}
        <li class="px-4 py-6 text-center text-sm text-muted-foreground/60">
          No commands found
        </li>
      {:else}
        {#each filtered as cmd, i (cmd.id)}
          <li
            id="cmd-{cmd.id}"
            role="option"
            aria-selected={i === selectedIndex}
          >
            <button
              type="button"
              class={cn(
                "flex w-full items-center gap-3 px-4 py-2 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground",
                i === selectedIndex && "bg-accent text-accent-foreground"
              )}
              onclick={() => { cmd.onRun(); onClose?.(); }}
              onmouseenter={() => { selectedIndex = i; }}
            >
              <span class="flex-1 truncate">{cmd.label}</span>
              {#if cmd.shortcut}
                <kbd class="shrink-0 rounded border border-border bg-muted px-1.5 py-0.5 text-[0.6rem] text-muted-foreground/70">
                  {cmd.shortcut}
                </kbd>
              {/if}
            </button>
          </li>
        {/each}
      {/if}
    </ul>
  </div>
{/if}
