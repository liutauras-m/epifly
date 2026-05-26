<script lang="ts">
  import ArrowUpIcon from "@lucide/svelte/icons/arrow-up";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import SquareIcon from "@lucide/svelte/icons/square";
  import { cn } from "../../utils/cn.js";
  import * as Button from "../ui/button/index.js";
  import * as Textarea from "../ui/textarea/index.js";

  type Props = {
    disabled?: boolean;
    isStreaming?: boolean;
    placeholder?: string;
    helperText?: string;
    class?: string;
    onSubmit?: (value: string) => void | Promise<void>;
    onStop?: () => void;
    onAttach?: () => void;
  };

  let {
    disabled = false,
    isStreaming = false,
    placeholder = "Message...",
    helperText,
    class: className,
    onSubmit,
    onStop,
    onAttach
  }: Props = $props();

  let message = $state("");
  let textareaEl = $state<HTMLTextAreaElement | null>(null);

  let canSend = $derived(message.trim().length > 0 && !isStreaming && !disabled);

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSend) return;
    const trimmed = message.trim();
    message = "";
    onSubmit?.(trimmed);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!canSend) return;
      const trimmed = message.trim();
      message = "";
      onSubmit?.(trimmed);
    }
  }

  // Auto-resize textarea
  $effect(() => {
    if (textareaEl) {
      textareaEl.style.height = "auto";
      textareaEl.style.height = `${textareaEl.scrollHeight}px`;
    }
  });
</script>

<form
  onsubmit={handleSubmit}
  class={cn(
    "sticky bottom-0 z-30 space-y-2 bg-gradient-to-t from-background via-background/95 to-background/0 px-3 pb-[calc(0.75rem+var(--safe-bottom))] pt-6 backdrop-blur-xl sm:px-6",
    className
  )}
>
  <div
    class={cn(
      "app-chat-composer-shell mx-auto flex min-h-14 w-full max-w-3xl items-end gap-2 rounded-[calc(var(--radius-app)+1rem)] border border-transparent bg-background/95 px-2 py-2 shadow-sm focus-within:border-ring/50 focus-within:ring-3 focus-within:ring-ring/15",
      disabled && "bg-muted/25 text-muted-foreground"
    )}
  >
    <Button.Button
      type="button"
      variant="ghost"
      size="icon-lg"
      disabled={disabled || isStreaming || !onAttach}
      onclick={onAttach}
      aria-label="Attach file"
      class="mb-0 size-11 rounded-full text-foreground transition-transform duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:scale-105 hover:bg-transparent hover:text-foreground active:scale-95 disabled:text-foreground disabled:opacity-100"
    >
      <PlusIcon class="size-7" strokeWidth={2.25} aria-hidden="true" />
    </Button.Button>

    <Textarea.Textarea
      bind:ref={textareaEl}
      bind:value={message}
      {placeholder}
      disabled={disabled || isStreaming}
      rows={1}
      onkeydown={handleKeydown}
      aria-describedby={helperText ? "chat-composer-helper" : undefined}
      class="max-h-44 min-h-10 flex-1 resize-none border-0 bg-transparent px-0 py-2 text-base leading-relaxed shadow-none outline-none focus-visible:ring-0 focus-visible:ring-offset-0 disabled:cursor-not-allowed disabled:bg-transparent disabled:opacity-100 sm:text-sm"
    />

    {#if isStreaming}
      <Button.Button
        type="button"
        size="icon-sm"
        onclick={onStop}
        aria-label="Stop generating"
        class="mb-1 rounded-full shadow-none"
      >
        <SquareIcon size={15} strokeWidth={1.75} aria-hidden="true" />
      </Button.Button>
    {:else}
      <Button.Button
        type="submit"
        size="icon-lg"
        disabled={!canSend}
        aria-label="Send message"
        class="mb-0 size-11 rounded-full bg-foreground text-background shadow-sm transition-transform duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:scale-105 hover:bg-foreground/90 active:scale-95 disabled:bg-foreground disabled:text-background disabled:opacity-100"
      >
        <ArrowUpIcon size={21} strokeWidth={2} aria-hidden="true" />
      </Button.Button>
    {/if}
  </div>

  {#if helperText}
    <p id="chat-composer-helper" class="mx-auto max-w-3xl px-3 text-xs leading-5 text-muted-foreground">
      {helperText}
    </p>
  {/if}
</form>
