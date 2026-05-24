<script lang="ts">
  import type { InvoiceData, RoutingMeta } from '@conusai/sdk';
  import ToolCallCard from './ToolCallCard.svelte';
  import HostedProjectCard from './HostedProjectCard.svelte';
  import CapabilityPinChip from './CapabilityPinChip.svelte';
  import MessageList from '../components/MessageList.svelte';
  import MessageBubble from '../components/MessageBubble.svelte';
  import ThinkingIndicator from '../components/ThinkingIndicator.svelte';

  // ChatMessage is defined in MessageList to avoid circular dependency.
  // Import here for local use; re-export for backward compatibility with consumers.
  import type { ChatMessage } from '../components/MessageList.svelte';
  export type { ChatMessage };

  export interface ToolCardEntry {
    name: string;
    status: 'running' | 'success' | 'error';
    result: string;
    startTime: number;
    /** Public URL returned by host_project when hosting_type = "static". */
    publicUrl?: string;
    /** Workspace-relative project path. */
    projectPath?: string;
    /** Framework name, e.g. "sveltekit". */
    framework?: string;
  }

  let {
    messages,
    toolCards,
    toolCardsList,
    invoiceResults = new Map(),
    inFlight = false,
    routingMeta = null,
    onRetryWithCapability = undefined,
    messagesEl = $bindable<HTMLElement | undefined>(),
  }: {
    messages: ChatMessage[];
    toolCards: Map<string, ToolCardEntry>;
    // Optional pre-flattened array — emitted by `createChatStream` to bypass
    // the Map-in-prop reactivity gap in Svelte 5 (see comment in
    // `createChatStream.svelte.ts → api.toolCardsList`). When present, the
    // template iterates this instead of the Map.
    toolCardsList?: Array<[string, ToolCardEntry]>;
    invoiceResults?: Map<string, unknown>;
    inFlight?: boolean;
    /** Routing metadata from the current turn — shows a CapabilityPinChip when forced (PR 3.B). */
    routingMeta?: RoutingMeta | null;
    /**
     * If provided, render a "Retry with explicit capability" button below the
     * last assistant message when the gateway returned no tools or the
     * assistant signalled it doesn't have a needed tool (PR 3.B.2). The parent
     * is responsible for opening the picker + re-sending with `forcedCapability`.
     */
    onRetryWithCapability?: () => void;
    messagesEl?: HTMLElement;
  } = $props();

  // Prefer the explicit list when the caller provides one.
  const cards = $derived(
    toolCardsList ?? Array.from(toolCards?.entries() ?? []),
  );

  // ── Zero-tools detection (PR 3.B.2) ──────────────────────────────────────
  const NO_TOOL_REGEXES = [
    /(don['']?t|do not|no longer) have (the |a |any )?(\w+)\s+tool/i,
    /no tools (are )?available/i,
  ];
  const noToolsTurn = $derived.by(() => {
    if (!onRetryWithCapability || messages.length === 0 || inFlight) return false;
    const structuredZero =
      routingMeta != null &&
      routingMeta.selected_capabilities.length === 0 &&
      routingMeta.pinned_tools.length === 0;
    if (structuredZero) return true;
    const lastAi = [...messages].reverse().find((m) => m.role === 'ai');
    if (!lastAi || lastAi.streaming) return false;
    return NO_TOOL_REGEXES.some((re) => re.test(lastAi.text));
  });
  const retryButtonLabel = $derived(
    routingMeta?.forced_capability ? 'Pick another capability' : 'Retry with explicit capability'
  );
</script>

<MessageList {messages} bind:el={messagesEl}>
  {#snippet before()}
    {#if routingMeta}
      <div class="row pin-row">
        <CapabilityPinChip {routingMeta} />
      </div>
    {/if}
  {/snippet}

  {#snippet renderMessage(msg, _i)}
    {#if msg.role === 'thinking'}
      <ThinkingIndicator />
    {:else if msg.role === 'user'}
      <MessageBubble role="user" text={msg.text} />
    {:else if msg.text.startsWith('__invoice__')}
      {@const token = msg.text.slice('__invoice__'.length)}
      {@const inv = invoiceResults.get(token) as InvoiceData | undefined}
      {#if inv}
        <div class="row ai-row">
          <div class="ai-mark" aria-hidden="true">
            <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
              <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
              <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <div class="invoice-stub">
            <span class="inv-label">Invoice extracted</span>
            <span class="inv-total">{inv.currency ?? ''}{inv.total_amount != null ? Number(inv.total_amount).toFixed(2) : '—'}</span>
          </div>
        </div>
      {/if}
    {:else}
      <MessageBubble
        role="assistant"
        text={msg.text}
        streaming={msg.streaming}
        words={msg.words}
      />
    {/if}
  {/snippet}

  {#snippet after()}
    {#if noToolsTurn && onRetryWithCapability}
      <div class="row retry-row">
        <button type="button" class="retry-btn" onclick={() => onRetryWithCapability?.()}>
          {retryButtonLabel}
        </button>
      </div>
    {/if}

    {#each cards as [id, card] (id)}
      <div class="row tool-row">
        <ToolCallCard {id} name={card.name} status={card.status} result={card.result} startTime={card.startTime} />
      </div>
      {#if card.publicUrl && card.status === 'success'}
        <div class="row ai-row hosted-row">
          <div class="ai-mark" aria-hidden="true">
            <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
              <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
              <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <HostedProjectCard
            url={card.publicUrl}
            projectPath={card.projectPath ?? ''}
            framework={card.framework ?? ''}
          />
        </div>
      {/if}
    {/each}
  {/snippet}
</MessageList>

<style>
  /* ── Feature-level row layout (routing pin, tool cards, invoice, retry) ── */
  .row {
    display:     flex;
    align-items: flex-start;
    padding:     3px var(--space-4);
    gap:         var(--space-2);
  }

  .pin-row {
    padding-top:    var(--space-2);
    padding-bottom: var(--space-1);
  }

  .retry-row {
    justify-content: flex-start;
    padding-top:     var(--space-1);
    padding-bottom:  var(--space-2);
    padding-left:    calc(var(--space-4) + 24px); /* align with ai-bubble */
  }
  .retry-btn {
    display:       inline-flex;
    align-items:   center;
    padding:       4px 10px;
    border-radius: var(--radius-md);
    border:        1px solid color-mix(in srgb, var(--color-accent) 50%, transparent);
    background:    color-mix(in srgb, var(--color-accent) 12%, transparent);
    color:         var(--color-accent);
    font-family:   var(--font-family-sans);
    font-size:     var(--font-size-meta);
    cursor:        pointer;
    transition:    filter var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .retry-btn:hover { filter: brightness(1.08); }
  .retry-btn:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .ai-row {
    justify-content: flex-start;
    padding-right:   var(--space-8);
  }

  .tool-row {
    padding-right: var(--space-4);
  }

  .hosted-row {
    margin-top: var(--space-1);
  }

  /* ── AI avatar mark (for invoice stub row) ───────────────────────────── */
  .ai-mark {
    width:           22px;
    height:          22px;
    border-radius:   50%;
    background:      var(--color-bg-hover);
    color:           var(--color-fg-subtle);
    display:         flex;
    align-items:     center;
    justify-content: center;
    flex-shrink:     0;
    margin-top:      2px;
  }

  /* ── Invoice stub ─────────────────────────────────────────────────────── */
  .invoice-stub {
    display:     flex;
    gap:         var(--space-3);
    align-items: baseline;
    border:      1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding:     var(--space-2) var(--space-3);
    font-size:   var(--font-size-meta);
  }
  .inv-label { color: var(--color-fg-subtle); }
  .inv-total { font-family: var(--font-family-mono); font-weight: 600; }
</style>
