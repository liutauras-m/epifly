<svelte:options runes={true} />
<script lang="ts">
  /**
   * MessageList — scrollable chat message container with auto-scroll (Phase 4.2).
   *
   * Renders an array of ChatMessages using <MessageBubble> and <ThinkingIndicator>.
   * Features:
   *   - Auto-scrolls to bottom when new messages arrive, but only when the user
   *     is already near the bottom (scroll-lock-on-new-content behaviour).
   *   - IntersectionObserver "jump to latest" pill surfaces when the user scrolls
   *     away from the bottom.
   *   - `before` snippet: inject rows above messages (e.g., routing audit chips).
   *   - `after`  snippet: inject rows below messages (e.g., tool cards, retry buttons).
   *   - `renderMessage` snippet: override per-message rendering (e.g., invoice stubs).
   *
   * Usage (simple):
   *   <MessageList {messages} bind:el />
   *
   * Usage (with feature rows injected by AgentChatStream):
   *   <MessageList {messages} bind:el>
   *     {#snippet before()}...{/snippet}
   *     {#snippet after()}...{/snippet}
   *   </MessageList>
   */
  import type { Snippet } from 'svelte';
  import MessageBubble from './MessageBubble.svelte';
  import ThinkingIndicator from './ThinkingIndicator.svelte';

  /**
   * ChatMessage — canonical chat message type (Phase 4.2).
   * Defined here (components/MessageList) to avoid a circular dependency with
   * features/AgentChatStream (which imports MessageList). AgentChatStream
   * re-exports this type for backward compatibility.
   */
  export interface ChatMessage {
    role: 'user' | 'ai' | 'thinking';
    text: string;
    streaming?: boolean;
    words?: { t: string; id: number; delay: number }[];
  }

  let {
    messages = [],
    before,
    after,
    renderMessage,
    el = $bindable<HTMLElement | undefined>(),
  }: {
    messages: ChatMessage[];
    /** Inject rows before the message list (e.g., routing audit chip). */
    before?: Snippet;
    /** Inject rows after the message list (e.g., tool cards, retry button). */
    after?: Snippet;
    /**
     * Custom per-message renderer. When provided, MessageList delegates rendering
     * of each message to this snippet instead of the default MessageBubble /
     * ThinkingIndicator. Useful for feature-level overrides (invoice stubs, etc.).
     * Signature: (msg: ChatMessage, index: number) => Rendered
     */
    renderMessage?: Snippet<[ChatMessage, number]>;
    el?: HTMLElement;
  } = $props();

  // Jump-to-latest pill state
  let showJumpPill = $state(false);
  let sentinel: HTMLElement | undefined = $state();
  let scrollEl: HTMLElement | undefined = $state();

  // IntersectionObserver — hide pill when bottom sentinel is visible
  $effect(() => {
    if (!sentinel || !scrollEl) return;
    const obs = new IntersectionObserver(
      ([entry]) => { showJumpPill = !entry.isIntersecting; },
      { root: scrollEl, threshold: 0.1 }
    );
    obs.observe(sentinel);
    return () => obs.disconnect();
  });

  // Auto-scroll on new messages, but only when already near the bottom
  $effect(() => {
    void messages.length;
    if (!showJumpPill && scrollEl) {
      requestAnimationFrame(() => {
        scrollEl?.scrollTo({ top: scrollEl.scrollHeight, behavior: 'smooth' });
      });
    }
  });

  function scrollToBottom() {
    scrollEl?.scrollTo({ top: scrollEl.scrollHeight, behavior: 'smooth' });
    showJumpPill = false;
  }

  // Expose the inner scroll element via the `el` binding
  $effect(() => { el = scrollEl; });
</script>

<div class="message-list-wrap">
  <div class="message-list" bind:this={scrollEl} role="log" aria-live="polite">
    {#if before}
      {@render before()}
    {/if}

    {#each messages as msg, i (i)}
      {#if renderMessage}
        {@render renderMessage(msg, i)}
      {:else if msg.role === 'thinking'}
        <ThinkingIndicator />
      {:else}
        <MessageBubble
          role={msg.role === 'ai' ? 'assistant' : 'user'}
          text={msg.text}
          streaming={msg.streaming}
          words={msg.words}
        />
      {/if}
    {/each}

    {#if after}
      {@render after()}
    {/if}

    <!-- Sentinel element — IntersectionObserver detects when this is out of view -->
    <div bind:this={sentinel} class="bottom-sentinel" aria-hidden="true"></div>
    <div class="bottom-spacer" aria-hidden="true"></div>
  </div>

  {#if showJumpPill}
    <button
      class="jump-pill"
      type="button"
      onclick={scrollToBottom}
      aria-label="Jump to latest message"
    >
      ↓ Latest
    </button>
  {/if}
</div>

<style>
  /* ── Wrapper — provides the positioning context for the jump pill ──────── */
  .message-list-wrap {
    position:       relative;
    flex:           1;
    min-height:     0;
    display:        flex;
    flex-direction: column;
  }

  /* ── Scroll container ─────────────────────────────────────────────────── */
  .message-list {
    flex:                      1;
    overflow-y:                auto;
    overscroll-behavior:       contain;
    -webkit-overflow-scrolling: touch;
    padding:                   var(--space-4) 0 var(--space-2);
    display:                   flex;
    flex-direction:            column;
    /* Messages anchor to the bottom — empty space appears above, not below */
    justify-content:           flex-end;
    gap:                       var(--_list-gap, 2px);
    /* Once there's enough content to scroll, justify-content has no effect — correct */
    min-height:                0;
  }

  .bottom-sentinel { height: 0; flex-shrink: 0; }
  .bottom-spacer   { height: var(--space-4); flex-shrink: 0; }

  /* ── Jump to latest pill ─────────────────────────────────────────────── */
  .jump-pill {
    position:      absolute;
    bottom:        var(--space-5);
    left:          50%;
    transform:     translateX(-50%);
    z-index:       10;
    padding:       var(--space-2) var(--space-4);
    border-radius: var(--radius-full);
    border:        1px solid var(--color-border);
    background:    var(--color-bg-raised);
    color:         var(--color-fg);
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-meta);
    cursor:        pointer;
    white-space:   nowrap;
    box-shadow:    0 2px 8px color-mix(in srgb, var(--color-fg) 10%, transparent);
    transition:    background var(--duration-fast) var(--ease-standard),
                   opacity    var(--duration-fast) var(--ease-standard);  /* [feedback] pill appear/disappear */
  }
  .jump-pill:hover { background: var(--color-bg-hover); }
  .jump-pill:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  @media (prefers-reduced-motion: reduce) {
    .jump-pill { transition: none; }
  }
</style>
