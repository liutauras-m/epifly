<script lang="ts">
  import type { InvoiceData } from '@conusai/sdk';
  import ToolCallCard from './ToolCallCard.svelte';
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
  }

  let {
    messages,
    toolCards,
    toolCardsList,
    invoiceResults = new Map(),
    inFlight = false,
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
    messagesEl?: HTMLElement;
  } = $props();

  // Prefer the explicit list when the caller provides one.
  const cards = $derived(
    toolCardsList ?? Array.from(toolCards?.entries() ?? []),
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
  {#each messages as msg, i (i)}
    {#if msg.role === 'thinking'}
      <div class="row ai-row message ai thinking">
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
      <div class="row user-row message user">
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
      <div class="row ai-row message ai" class:streaming={msg.streaming}>
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

  {#each cards as [id, card] (id)}
    <div class="row tool-row">
      <ToolCallCard {id} name={card.name} status={card.status} result={card.result} startTime={card.startTime} />
    </div>
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
    padding: var(--s-4) 0 var(--s-2);
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
    padding: 3px var(--s-4);
    gap: var(--s-2);
  }

  .user-row {
    justify-content: flex-end;
    padding-left: var(--s-8);
  }

  .ai-row {
    justify-content: flex-start;
    padding-right: var(--s-8);
  }

  .tool-row {
    padding-right: var(--s-4);
  }

  /* ── User bubble ─────────────────────────────────── */
  .user-bubble {
    background: var(--ember);
    color: #fff;
    border-radius: 18px 18px 4px 18px;
    padding: 10px 14px;
    font-size: var(--t-body);
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
    max-width: min(480px, 78vw);
  }

  /* ── AI avatar mark ──────────────────────────────── */
  .ai-mark {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--paper-3);
    color: var(--ink-3);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 2px;
  }

  /* ── AI bubble ───────────────────────────────────── */
  .ai-bubble {
    font-size: var(--t-body);
    line-height: 1.65;
    color: var(--ink);
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
    font-family: var(--font-display);
    font-size: var(--t-h2);
    margin: 0.6em 0 0.3em;
    color: var(--ink);
  }
  .ai-bubble :global(.md-h2) {
    font-size: var(--t-body);
    font-weight: 700;
    margin: 0.8em 0 0.25em;
  }
  .ai-bubble :global(.md-h3) {
    font-size: var(--t-body);
    font-weight: 600;
    color: var(--ink-2);
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
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    padding: var(--s-3);
    overflow-x: auto;
    margin: 0.5em 0;
  }
  .ai-bubble :global(.md-pre code) {
    font-family: var(--font-mono);
    font-size: var(--t-meta);
    white-space: pre;
  }
  .ai-bubble :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .ai-bubble :global(.md-hr) {
    border: none;
    border-top: 1px solid var(--rule);
    margin: 0.75em 0;
  }
  .ai-bubble :global(strong) { font-weight: 700; }
  .ai-bubble :global(em) { font-style: italic; }

  /* Streaming word animation */
  .ai-text { display: inline; }
  .tok { display: inline; animation: tok-in 120ms ease both; }
  @keyframes tok-in { from { opacity: 0; } to { opacity: 1; } }
  .stream-cursor {
    display: inline-block;
    width: 2px;
    height: 1em;
    background: var(--ember);
    margin-left: 2px;
    animation: blink 1s step-end infinite;
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
    background: var(--ember);
  }
  .sonar-ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1.5px solid var(--ember);
    animation: sonar-out 1.8s ease-out infinite;
  }
  .sonar-r2 { animation-delay: 0.6s; }
  @keyframes sonar-out {
    0%   { transform: scale(0.3); opacity: 0.9; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  /* ── Invoice stub ────────────────────────────────── */
  .invoice-stub {
    display: flex;
    gap: var(--s-3);
    align-items: baseline;
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    padding: var(--s-2) var(--s-3);
    font-size: var(--t-meta);
  }
  .inv-label { color: var(--ink-3); }
  .inv-total { font-family: var(--font-mono); font-weight: 600; }

  /* ── Bottom spacer ───────────────────────────────── */
  .bottom-spacer { height: var(--s-4); flex-shrink: 0; }

  /* ── Mobile tweaks ───────────────────────────────── */
  @media (max-width: 640px) {
    .user-row { padding-left: var(--s-7); }
    .ai-row   { padding-right: var(--s-6); }
    .user-bubble { max-width: min(340px, 80vw); }
  }
</style>
