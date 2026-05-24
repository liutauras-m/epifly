<svelte:options runes={true} />
<script lang="ts">
  /**
   * MessageBubble — renders a single chat message bubble (Phase 4.2).
   *
   * Handles user messages (right-aligned, accent fill) and assistant messages
   * (left-aligned, AI avatar mark + markdown prose + streaming animation).
   *
   * Per design spec §8.2: user bubble uses accent fill with asymmetric radii
   * (18px 18px 4px 18px — ember left rail). Assistant messages use the AI mark
   * with optional streaming word-by-word reveal.
   *
   * Usage:
   *   <MessageBubble role="user" text="Hello" />
   *   <MessageBubble role="assistant" text="Hi there" />
   *   <MessageBubble role="assistant" text={partial} streaming={true} words={wordTokens} />
   */
  import { renderMarkdown } from '../utils/md.js';

  export interface MessageWord {
    t: string;
    id: number;
    delay: number;
  }

  let {
    role,
    text,
    streaming = false,
    words,
  }: {
    /** 'user' renders a right-aligned accent bubble; 'assistant' renders left-aligned with AI mark. */
    role: 'user' | 'assistant';
    text: string;
    /** True while the assistant is actively streaming tokens. */
    streaming?: boolean;
    /** Word tokens for streaming reveal animation. */
    words?: MessageWord[];
  } = $props();
</script>

{#if role === 'user'}
  <div class="row user-row">
    <div class="user-bubble">{text}</div>
  </div>
{:else}
  <div class="row ai-row" class:streaming>
    <div class="ai-mark" aria-hidden="true">
      <svg viewBox="0 0 16 16" fill="none" width="16" height="16">
        <circle cx="8" cy="8" r="7" stroke="currentColor" stroke-width="1.5"/>
        <path d="M5 8.5l2 2 4-4" stroke="currentColor" stroke-width="1.5"
              stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>
    <div class="ai-bubble" aria-live="polite">
      {#if streaming && words}
        <span class="ai-text">
          {#each words as w (w.id)}<span class="tok" style="animation-delay:{w.delay}ms">{w.t}</span>{/each}{#if text}<span class="stream-cursor" aria-hidden="true"></span>{:else}&nbsp;<span class="sonar sonar-sm" role="status" aria-label="Generating"><span class="sonar-ring sonar-r1"></span><span class="sonar-ring sonar-r2"></span><span class="sonar-core"></span></span>{/if}
        </span>
      {:else}
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        {@html renderMarkdown(text)}
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ── Row layout ──────────────────────────────────────────────────────────── */
  .row {
    display:     flex;
    align-items: flex-start;
    padding:     var(--_row-v, 3px) var(--space-4);
    gap:         var(--space-2);
  }

  .user-row {
    justify-content: flex-end;
    padding-left:    var(--space-8);
  }

  .ai-row {
    justify-content: flex-start;
    padding-right:   var(--space-8);
  }

  /* ── User bubble ─────────────────────────────────────────────────────────── */
  /* Design spec §8.2: accent fill, asymmetric radii (ember left rail) */
  .user-bubble {
    background:    var(--color-accent);
    color:         var(--color-on-accent);
    border-radius: var(--_bubble-r, 18px) var(--_bubble-r, 18px) var(--radius-xs) var(--_bubble-r, 18px);
    padding:       var(--_bubble-pv, 10px) var(--_bubble-ph, 14px);
    font-size:     var(--font-size-body);
    line-height:   1.55;
    white-space:   pre-wrap;
    word-break:    break-word;
    max-width:     min(480px, 78cqi);
  }

  /* ── AI avatar mark ──────────────────────────────────────────────────────── */
  .ai-mark {
    width:           var(--_mark-size, 22px);
    height:          var(--_mark-size, 22px);
    border-radius:   50%;
    background:      var(--color-bg-hover);
    color:           var(--color-fg-subtle);
    display:         flex;
    align-items:     center;
    justify-content: center;
    flex-shrink:     0;
    margin-top:      2px;
    transition:      color       var(--duration-fast) var(--ease-standard),
                     background  var(--duration-fast) var(--ease-standard);
  }

  /* [continuity] Traveling-ember pulse on AI mark during streaming */
  .ai-row.streaming .ai-mark {
    color:      var(--color-accent);
    background: var(--color-accent-soft);
    animation:  ember-pulse 1.4s ease-in-out infinite;  /* [continuity] response in progress */
  }
  @keyframes ember-pulse {
    0%, 100% { opacity: 0.5; }
    50%      { opacity: 1; }
  }

  /* ── AI bubble prose ─────────────────────────────────────────────────────── */
  .ai-bubble {
    font-size:   var(--font-size-body);
    line-height: 1.65;
    color:       var(--color-fg);
    word-break:  break-word;
    min-width:   0;
  }

  /* Markdown element styles scoped inside ai-bubble */
  .ai-bubble :global(.md-p)            { margin: 0 0 0.75em; }
  .ai-bubble :global(.md-p:last-child) { margin-bottom: 0; }
  .ai-bubble :global(.md-h1) {
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-h2);
    margin:      0.6em 0 0.3em;
    color:       var(--color-fg);
  }
  .ai-bubble :global(.md-h2) {
    font-size:   var(--font-size-body);
    font-weight: 700;
    margin:      0.8em 0 0.25em;
  }
  .ai-bubble :global(.md-h3) {
    font-size:   var(--font-size-body);
    font-weight: 600;
    color:       var(--color-fg-muted);
    margin:      0.6em 0 0.2em;
  }
  .ai-bubble :global(.md-ul),
  .ai-bubble :global(.md-ol) {
    margin:         0.4em 0 0.75em 1.2em;
    padding:        0;
    display:        flex;
    flex-direction: column;
    gap:            0.2em;
  }
  .ai-bubble :global(.md-ul li),
  .ai-bubble :global(.md-ol li) { line-height: 1.55; }
  .ai-bubble :global(.md-pre) {
    background:    var(--color-bg-raised);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding:       var(--space-3);
    overflow-x:    auto;
    margin:        0.5em 0;
  }
  .ai-bubble :global(.md-pre code) {
    font-family: var(--font-family-mono);
    font-size:   var(--font-size-meta);
    white-space: pre;
  }
  .ai-bubble :global(code) {
    font-family:   var(--font-family-mono);
    font-size:     0.88em;
    background:    var(--color-bg-raised);
    border:        1px solid var(--color-border);
    border-radius: var(--_code-r, 4px);
    padding:       var(--_code-pv, 1px) var(--_code-ph, 5px);
  }
  .ai-bubble :global(.md-hr) {
    border:     none;
    border-top: 1px solid var(--color-border);
    margin:     0.75em 0;
  }
  .ai-bubble :global(strong) { font-weight: 700; }
  .ai-bubble :global(em)     { font-style: italic; }
  .ai-bubble :global(.md-link) {
    color:                 var(--color-accent);
    text-decoration:       underline;
    text-underline-offset: 2px;
    border-radius:         var(--_link-r, 2px);
    transition:            color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }
  .ai-bubble :global(.md-link:hover) {
    color: var(--color-accent-dim, var(--color-accent));
  }
  .ai-bubble :global(.md-link:focus-visible) {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* ── Streaming animations ────────────────────────────────────────────────── */
  .ai-text { display: inline; }
  /* [feedback] Word-by-word reveal during streaming */
  .tok { display: inline; animation: tok-in 120ms ease both; }  /* [feedback] */
  @keyframes tok-in { from { opacity: 0; } to { opacity: 1; } }
  .stream-cursor {
    display:        inline-block;
    width:          var(--_cursor-w, 2px);
    height:         1em;
    background:     var(--color-accent);
    margin-left:    2px;
    animation:      blink 1s step-end infinite;  /* [feedback] cursor blink */
    vertical-align: text-bottom;
  }
  @keyframes blink { 50% { opacity: 0; } }

  /* Inline sonar (mid-stream idle indicator, before first word) */
  .sonar {
    display:     inline-flex;
    position:    relative;
    width:       var(--_sonar-size, 14px);
    height:      var(--_sonar-size, 14px);
    flex-shrink: 0;
    align-self:  center;
  }
  .sonar-sm { width: var(--_sonar-sm, 10px); height: var(--_sonar-sm, 10px); }
  .sonar-core {
    position:      absolute;
    inset:         25%;
    border-radius: 50%;
    background:    var(--color-accent);
  }
  .sonar-ring {
    position:      absolute;
    inset:         0;
    border-radius: 50%;
    border:        1.5px solid var(--color-accent);
    animation:     sonar-out 1.8s ease-out infinite;  /* [feedback] */
  }
  .sonar-r2 { animation-delay: 0.6s; }
  @keyframes sonar-out {
    0%   { transform: scale(0.3); opacity: 0.9; }
    100% { transform: scale(2.2); opacity: 0; }
  }

  /* ── Reduced-motion ──────────────────────────────────────────────────────── */
  @media (prefers-reduced-motion: reduce) {
    .ai-row.streaming .ai-mark { animation: none !important; }
    .tok                       { animation: none; }
    .stream-cursor             { animation: none; }
    .sonar-ring                { animation: none; opacity: 0.6; }
  }

  /* ── Compact tweaks (container query — no viewport dependency) ───────────── */
  @container app-shell (max-width: 639px) {
    .user-row    { padding-left: var(--space-7); }
    .ai-row      { padding-right: var(--space-6); }
    .user-bubble { max-width: min(340px, 80cqi); }
  }
</style>
