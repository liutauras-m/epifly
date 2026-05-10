<script lang="ts">
  import type { InvoiceData } from '@conusai/sdk';
  import ToolCallCard from './ToolCallCard.svelte';

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
    invoiceResults = new Map(),
    inFlight = false,
    messagesEl = $bindable<HTMLElement | undefined>(),
  }: {
    messages: ChatMessage[];
    toolCards: Map<string, ToolCardEntry>;
    invoiceResults?: Map<string, unknown>;
    inFlight?: boolean;
    messagesEl?: HTMLElement;
  } = $props();
</script>

<div class="messages" bind:this={messagesEl} role="log" aria-live="polite">
  {#each messages as msg, i (i)}
    {#if msg.role === 'thinking'}
      <div class="message ai thinking">
        <span class="sonar" role="status" aria-label="Waiting">
          <span class="sonar-ring sonar-r1"></span>
          <span class="sonar-ring sonar-r2"></span>
          <span class="sonar-core"></span>
        </span>
      </div>
    {:else if msg.role === 'user'}
      <div class="message user">{msg.text}</div>
    {:else if msg.text.startsWith('__invoice__')}
      {@const token = msg.text.slice('__invoice__'.length)}
      {@const inv = invoiceResults.get(token) as InvoiceData | undefined}
      {#if inv}
        <div class="message ai invoice-stub">
          <span class="inv-label">Invoice extracted</span>
          <span class="inv-total">{inv.currency ?? ''}{inv.total_amount != null ? Number(inv.total_amount).toFixed(2) : '—'}</span>
        </div>
      {/if}
    {:else}
      <div class="message ai" class:streaming={msg.streaming}>
        {#if msg.streaming && msg.words}
          <span class="ai-text" aria-live="polite">
            {#each msg.words as w (w.id)}<span class="tok" style="animation-delay:{w.delay}ms">{w.t}</span>{/each}
            {#if msg.text}<span class="stream-cursor" aria-hidden="true"></span>{:else}&nbsp;
              <span class="sonar sonar-sm" role="status" aria-label="Waiting">
                <span class="sonar-ring sonar-r1"></span>
                <span class="sonar-ring sonar-r2"></span>
                <span class="sonar-core"></span>
              </span>
            {/if}
          </span>
        {:else}
          <span class="ai-text">{msg.text}</span>
        {/if}
      </div>
    {/if}
  {/each}

  {#each [...toolCards.entries()] as [id, card] (id)}
    <ToolCallCard {id} name={card.name} status={card.status} result={card.result} startTime={card.startTime} />
  {/each}

  {#if !inFlight}
    <div class="chat-end-dot">
      <span class="sonar" role="status" aria-label="Ready">
        <span class="sonar-ring sonar-r1"></span>
        <span class="sonar-ring sonar-r2"></span>
        <span class="sonar-core"></span>
      </span>
    </div>
  {/if}
</div>

<style>
  .messages {
    flex: 1; overflow-y: auto; padding: var(--s-5) var(--s-6);
    display: flex; flex-direction: column; gap: var(--s-4);
  }
  .message { max-width: 72ch; line-height: 1.6; }
  .message.user {
    align-self: flex-end;
    background: var(--ember-soft); border: 1px solid var(--ember-glow);
    border-radius: var(--r-md); padding: var(--s-3) var(--s-4);
    white-space: pre-wrap;
  }
  .message.ai { align-self: flex-start; white-space: pre-wrap; }
  .ai-text { display: inline; }
  .tok { display: inline; animation: tok-in 120ms ease both; }
  @keyframes tok-in { from { opacity: 0; } to { opacity: 1; } }
  .stream-cursor {
    display: inline-block; width: 2px; height: 1em;
    background: var(--ember); margin-left: 1px; animation: blink 1s step-end infinite;
    vertical-align: text-bottom;
  }
  @keyframes blink { 50% { opacity: 0; } }
  .chat-end-dot { display: flex; padding: var(--s-2) 0; }
  .sonar { display: inline-flex; position: relative; width: 12px; height: 12px; flex-shrink: 0; }
  .sonar-sm { width: 8px; height: 8px; }
  .sonar-core {
    position: absolute; inset: 25%; border-radius: 50%;
    background: var(--ember);
  }
  .sonar-ring {
    position: absolute; inset: 0; border-radius: 50%;
    border: 1.5px solid var(--ember); animation: sonar-out 1.8s ease-out infinite;
  }
  .sonar-r2 { animation-delay: 0.6s; }
  @keyframes sonar-out {
    0%   { transform: scale(0.3); opacity: 0.9; }
    100% { transform: scale(2.2); opacity: 0; }
  }
  .invoice-stub {
    display: flex; gap: var(--s-3); align-items: baseline;
    border: 1px solid var(--rule); border-radius: var(--r-sm);
    padding: var(--s-2) var(--s-3); font-size: var(--t-meta);
  }
  .inv-label { color: var(--ink-3); }
  .inv-total { font-family: var(--font-mono); font-weight: 600; }
</style>
