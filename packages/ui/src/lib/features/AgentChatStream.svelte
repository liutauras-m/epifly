<script lang="ts">
  import type { InvoiceData, RoutingMeta } from '@conusai/sdk';
  import ToolCallCard from './ToolCallCard.svelte';
  import HostedProjectCard from './HostedProjectCard.svelte';
  import CapabilityPinChip from './CapabilityPinChip.svelte';
  import { renderMarkdown } from '../utils/md.js';

  export interface ChatMessage {
    role: 'user' | 'ai' | 'thinking';
    text: string;
    streaming?: boolean;
    words?: { t: string; id: number; delay: number }[];
  }

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
  // The structured signal is preferred — count = 0 means the router served no
  // tools this turn. The regex fallback catches the LLM saying "I don't have
  // X tool" even when tools were served (e.g. wrong tool was picked).
  const NO_TOOL_REGEXES = [
    /(don['’]?t|do not|no longer) have (the |a |any )?(\w+)\s+tool/i,
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

  function scrollToBottom() {
    messagesEl?.scrollTo({ top: messagesEl.scrollHeight, behavior: 'smooth' });
  }

  $effect(() => {
    // Re-run whenever messages or toolCards change.
    void messages.length;
    void cards.length;
    // Use RAF so the DOM has painted new content before measuring scrollHeight.
    requestAnimationFrame(() => scrollToBottom());
  });
</script>

<div class="messages" bind:this={messagesEl} role="log" aria-live="polite">
  <!-- Routing-audit chip: render whenever the gateway emitted routing_meta
       (every turn, post-PR 3.B). Click expands a popover with selected
       capabilities / pinned tools / lexical hits / max_score. -->
  {#if routingMeta}
    <div class="row pin-row">
      <CapabilityPinChip {routingMeta} />
    </div>
  {/if}

  {#each messages as msg, i (i)}
    {#if msg.role === 'thinking'}
      <div class="row ai-row thinking-row">
        <div class="ai-mark" aria-hidden="true">
          <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
            <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
            <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <span class="sonar" role="status" aria-label="Thinking">
          <span class="sonar-ring sonar-r1"></span>
          <span class="sonar-ring sonar-r2"></span>
          <span class="sonar-core"></span>
        </span>
      </div>

    {:else if msg.role === 'user'}
      <div class="row user-row">
        <div class="user-bubble">{msg.text}</div>
      </div>

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
      <div class="row ai-row" class:streaming={msg.streaming}>
        <div class="ai-mark" aria-hidden="true">
          <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
            <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
            <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="ai-bubble" aria-live="polite">
          {#if msg.streaming && msg.words}
            <span class="ai-text">
              {#each msg.words as w (w.id)}<span class="tok" style="animation-delay:{w.delay}ms">{w.t}</span>{/each}{#if msg.text}<span class="stream-cursor" aria-hidden="true"></span>{:else}&nbsp;<span class="sonar sonar-sm" role="status" aria-label="Generating"><span class="sonar-ring sonar-r1"></span><span class="sonar-ring sonar-r2"></span><span class="sonar-core"></span></span>{/if}
            </span>
          {:else}
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            {@html renderMarkdown(msg.text)}
          {/if}
        </div>
      </div>
    {/if}
  {/each}

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

  <!-- Bottom spacer for overscroll clearance -->
  <div class="bottom-spacer" aria-hidden="true"></div>
</div>

<style>
  .messages {
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
    padding: var(--space-4) 0 var(--space-2);
    display: flex;
    flex-direction: column;
    /* Messages anchor to the bottom — empty space appears above, not below */
    justify-content: flex-end;
    gap: 2px;
    /* Once there's enough content to scroll, justify-content loses effect — correct behavior */
    min-height: 0;
  }

  /* ── Row layout ──────────────────────────────────── */
  .row {
    display: flex;
    align-items: flex-start;
    padding: 3px var(--space-4);
    gap: var(--space-2);
  }

  .pin-row {
    padding-top: var(--space-2);
    padding-bottom: var(--space-1);
  }

  .retry-row {
    justify-content: flex-start;
    padding-top: var(--space-1);
    padding-bottom: var(--space-2);
    padding-left: calc(var(--space-4) + 24px); /* align with ai-bubble */
  }
  .retry-btn {
    display: inline-flex;
    align-items: center;
    padding: 4px 10px;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--color-accent) 50%, transparent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
    font-family: var(--font-family-sans);
    font-size: var(--font-size-meta);
    cursor: pointer;
    transition: filter var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .retry-btn:hover { filter: brightness(1.08); }
  .retry-btn:focus-visible {
    outline: var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .user-row {
    justify-content: flex-end;
    padding-left: var(--space-8);
  }

  .ai-row {
    justify-content: flex-start;
    padding-right: var(--space-8);
  }

  /* Thinking indicator row — same layout as ai-row but with the sonar ring */
  .thinking-row {
    justify-content: flex-start;
    padding-right: var(--space-8);
  }

  .tool-row {
    padding-right: var(--space-4);
  }

  /* ── User bubble ─────────────────────────────────── */
  /* Design spec §8.2: accent fill, asymmetric radii (ember left rail) */
  .user-bubble {
    background: var(--color-accent);
    color: #fff;
    border-radius: 18px 18px 4px 18px;
    padding: 10px 14px;
    font-size: var(--font-size-body);
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
    max-width: min(480px, 78cqi);  /* container query unit: no viewport dependency */
  }

  /* ── AI avatar mark ──────────────────────────────── */
  .ai-mark {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--color-bg-hover);
    color: var(--color-fg-subtle);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 2px;
  }

  /* ── AI bubble ───────────────────────────────────── */
  .ai-bubble {
    font-size: var(--font-size-body);
    line-height: 1.65;
    color: var(--color-fg);
    word-break: break-word;
    min-width: 0;
  }

  /* Markdown element styles scoped inside ai-bubble */
  .ai-bubble :global(.md-p) {
    margin: 0 0 0.75em;
  }
  .ai-bubble :global(.md-p:last-child) {
    margin-bottom: 0;
  }
  .ai-bubble :global(.md-h1) {
    font-family: var(--font-family-sans);
    font-size: var(--font-size-h2);
    margin: 0.6em 0 0.3em;
    color: var(--color-fg);
  }
  .ai-bubble :global(.md-h2) {
    font-size: var(--font-size-body);
    font-weight: 700;
    margin: 0.8em 0 0.25em;
  }
  .ai-bubble :global(.md-h3) {
    font-size: var(--font-size-body);
    font-weight: 600;
    color: var(--color-fg-muted);
    margin: 0.6em 0 0.2em;
  }
  .ai-bubble :global(.md-ul),
  .ai-bubble :global(.md-ol) {
    margin: 0.4em 0 0.75em 1.2em;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2em;
  }
  .ai-bubble :global(.md-ul li),
  .ai-bubble :global(.md-ol li) {
    line-height: 1.55;
  }
  .ai-bubble :global(.md-pre) {
    background: var(--color-bg-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--space-3);
    overflow-x: auto;
    margin: 0.5em 0;
  }
  .ai-bubble :global(.md-pre code) {
    font-family: var(--font-mono);
    font-size: var(--font-size-meta);
    white-space: pre;
  }
  .ai-bubble :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: var(--color-bg-raised);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .ai-bubble :global(.md-hr) {
    border: none;
    border-top: 1px solid var(--color-border);
    margin: 0.75em 0;
  }
  .ai-bubble :global(strong) { font-weight: 700; }
  .ai-bubble :global(em) { font-style: italic; }

  /* Streaming word animation [feedback] */
  .ai-text { display: inline; }
  .tok { display: inline; animation: tok-in 120ms ease both; }  /* [feedback] */
  @keyframes tok-in { from { opacity: 0; } to { opacity: 1; } }
  .stream-cursor {
    display: inline-block;
    width: 2px;
    height: 1em;
    background: var(--color-accent);
    margin-left: 2px;
    animation: blink 1s step-end infinite;  /* [feedback] */
    vertical-align: text-bottom;
  }
  @keyframes blink { 50% { opacity: 0; } }

  /* ── Thinking sonar ──────────────────────────────── */
  .sonar {
    display: inline-flex;
    position: relative;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    align-self: center;
  }
  .sonar-sm { width: 10px; height: 10px; }
  .sonar-core {
    position: absolute;
    inset: 25%;
    border-radius: 50%;
    background: var(--color-accent);
  }
  .sonar-ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1.5px solid var(--color-accent);
    animation: sonar-out 1.8s ease-out infinite;  /* [feedback] */
  }
  .sonar-r2 { animation-delay: 0.6s; }
  @keyframes sonar-out {
    0%   { transform: scale(0.3); opacity: 0.9; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  /* ── Hosted project row ─────────────────────────── */
  .hosted-row {
    margin-top: var(--space-1);
  }

  /* ── Markdown links ──────────────────────────────── */
  .ai-bubble :global(.md-link) {
    color: var(--color-accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    border-radius: 2px;
    transition: color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .ai-bubble :global(.md-link:hover) {
    color: var(--color-accent-dim, var(--color-accent));
  }
  .ai-bubble :global(.md-link:focus-visible) {
    outline: var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* ── Invoice stub ────────────────────────────────── */
  .invoice-stub {
    display: flex;
    gap: var(--space-3);
    align-items: baseline;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
    font-size: var(--font-size-meta);
  }
  .inv-label { color: var(--color-fg-subtle); }
  .inv-total { font-family: var(--font-mono); font-weight: 600; }

  /* ── Bottom spacer ───────────────────────────────── */
  .bottom-spacer { height: var(--space-4); flex-shrink: 0; }

  /* ── Compact tweaks (container query — no viewport dependency) ─── */
  @container app-shell (max-width: 639px) {
    .user-row { padding-left: var(--space-7); }
    .ai-row   { padding-right: var(--space-6); }
    .user-bubble { max-width: min(340px, 80cqi); }
  }
</style>
