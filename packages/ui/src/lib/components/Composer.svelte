<svelte:options runes={true} />
<script lang="ts">
  /**
   * Composer — message input (Phase 3.5).
   *
   * Promoted from AgentChatComposer.svelte with canonical name + enhanced states.
   * Lives inside AppShell's composer slot (which wraps it in <form aria-label="Message composer">).
   *
   * States: rest → focus (ember ring) → submitting (loading pulse) → error
   *
   * Attachments row uses Chip primitives with removable ✕.
   * visual-viewport listener keeps composer above the iOS software keyboard.
   *
   * Usage:
   *   <Composer onsubmit={send} placeholder="Ask anything…" />
   *   <Composer onsubmit={send} loading={inFlight} suggestions={chips} />
   *
   * Note: AgentChatComposer.svelte remains as a @deprecated shim that re-exports
   * this component — deleted at Phase 4 close via ui:contracts gate.
   */
  import type { Snippet } from 'svelte';
  import { autoGrow } from '../utils/actions.js';
  import { t } from '../utils/i18n.js';
  import Icon from './Icon.svelte';
  import Chip from './Chip.svelte';
  import { Send, Paperclip, X } from 'lucide-svelte';

  export type Attachment = {
    id:       string;
    name:     string;
    mimeType?: string;
  };

  let {
    value       = $bindable(''),
    placeholder = t('composer.placeholder'),
    loading     = false,
    disabled    = false,
    maxRows     = 8,
    attachments = $bindable([] as Attachment[]),
    onsubmit,
    onattach,
    onremoveattachment,
    onUpload,
    class: cls  = '',
  }: {
    value?:               string;
    placeholder?:         string;
    loading?:             boolean;
    disabled?:            boolean;
    maxRows?:             number;
    attachments?:         Attachment[];
    /** Called on submit with text + current attachments. */
    onsubmit?:            (value: string, attachments: Attachment[]) => void;
    onattach?:            () => void;
    onremoveattachment?:  (id: string) => void;
    /** Upload handler — receives File[] and returns resolved Attachment[]. */
    onUpload?:            (files: File[]) => Promise<Attachment[]>;
    class?:               string;
  } = $props();

  let textareaEl  = $state<HTMLTextAreaElement | undefined>(undefined);
  let composerEl  = $state<HTMLElement | undefined>(undefined);
  // iOS keyboard offset — set via visualViewport API (Phase 5.4)
  let keyboardOffset = $state(0);
  // [feedback] Chat send rebound — true for ~180ms after submit fires
  let rebounding  = $state(false);

  /**
   * Phase 5.4 — visual viewport listener.
   * iOS/Android: when the software keyboard appears, `window.visualViewport.height`
   * shrinks. We compute how much the composer needs to be lifted.
   * This keeps the send button visible above the keyboard at all times.
   */
  $effect(() => {
    if (typeof window === 'undefined' || !window.visualViewport) return;

    function onViewportResize() {
      const vv = window.visualViewport!;
      // Distance from bottom of visual viewport to bottom of layout viewport
      const offset = window.innerHeight - vv.height - vv.offsetTop;
      keyboardOffset = Math.max(0, offset);
    }

    window.visualViewport.addEventListener('resize', onViewportResize);
    window.visualViewport.addEventListener('scroll', onViewportResize);
    return () => {
      window.visualViewport!.removeEventListener('resize', onViewportResize);
      window.visualViewport!.removeEventListener('scroll', onViewportResize);
    };
  });

  function submit() {
    const trimmed = value.trim();
    if (!trimmed || loading || disabled) return;
    const atts = attachments.slice();
    onsubmit?.(trimmed, atts);
    value = '';
    attachments = [];
    // Reset textarea height
    if (textareaEl) {
      textareaEl.style.height = '';
    }
    // [feedback] Send rebound — scale 0.93 spring-snap back ~180ms
    rebounding = true;
    // Clear after animation completes so it can re-trigger on next send
    setTimeout(() => { rebounding = false; }, 220);
  }

  /** Focus the textarea — exposed so parent can call composerRef.focus(). */
  export function focus() {
    textareaEl?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  const canSend = $derived(value.trim().length > 0 && !loading && !disabled);
</script>

<div
  bind:this={composerEl}
  class="composer{loading ? ' composer-loading' : ''}{cls ? ` ${cls}` : ''}"
  style:padding-bottom={keyboardOffset > 0 ? `${keyboardOffset}px` : undefined}
  data-rebounding={rebounding || undefined}
>

  <!-- Attachments row -->
  {#if attachments.length > 0}
    <div class="composer-attachments" aria-label="Attached files">
      {#each attachments as att (att.id)}
        <Chip
          label={att.name}
          variant="tonal"
          size="sm"
          onremove={() => onremoveattachment?.(att.id)}
        />
      {/each}
    </div>
  {/if}

  <!-- Input row -->
  <div class="composer-row{rebounding ? ' composer-row-rebound' : ''}">

    <!-- Attach button -->
    {#if onattach}
      <button
        type="button"
        class="composer-btn"
        aria-label={t('composer.attach')}
        disabled={disabled || loading}
        onclick={onattach}
      >
        <Icon icon={Paperclip} size="md" />
      </button>
    {/if}

    <!-- Textarea -->
    <textarea
      bind:this={textareaEl}
      bind:value
      {placeholder}
      disabled={disabled || loading}
      rows={1}
      class="composer-input"
      aria-label="Message"
      aria-multiline="true"
      use:autoGrow={{ maxRows }}
      onkeydown={handleKeydown}
    ></textarea>

    <!-- Send button — never spins; the rebound is the send acknowledgement [feedback] -->
    <button
      type="submit"
      class="composer-send"
      aria-label={t('composer.send')}
      disabled={!canSend}
      onclick={submit}
    >
      <Icon icon={Send} size="md" />
    </button>

  </div>
</div>

<style>
  /* ── Container ───────────────────────────────────────────────────────────── */
  .composer {
    padding: var(--space-3) var(--space-4);
  }

  /* ── Attachments ─────────────────────────────────────────────────────────── */
  .composer-attachments {
    display:     flex;
    flex-wrap:   wrap;
    gap:         var(--space-1);
    padding-bottom: var(--space-2);
  }

  /* ── Row ─────────────────────────────────────────────────────────────────── */
  .composer-row {
    display:        flex;
    align-items:    flex-end;
    gap:            var(--space-2);
    background:     var(--color-bg-raised);
    border:         1px solid var(--color-border);
    border-radius:  var(--radius-md);
    padding:        var(--space-2) var(--space-2) var(--space-2) var(--space-3);

    transition:
      border-color var(--duration-fast) var(--ease-standard),  /* [feedback] */
      box-shadow   var(--duration-fast) var(--ease-standard);   /* [feedback] */
  }
  .composer-row:focus-within {
    border-color: var(--color-accent);
    box-shadow:   0 0 0 var(--focus-ring-offset) var(--color-bg),
                  0 0 0 calc(var(--focus-ring-offset) + 2px) var(--color-accent);
  }

  /* ── Textarea ────────────────────────────────────────────────────────────── */
  .composer-input {
    flex:           1;
    background:     transparent;
    border:         none;
    outline:        none;
    resize:         none;
    font-family:    var(--font-family-sans);
    font-size:      max(16px, var(--font-size-body));  /* prevent iOS zoom */
    color:          var(--color-fg);
    line-height:    1.5;
    padding:        var(--space-1) 0;
    min-height:     calc(1.5 * max(16px, var(--font-size-body)));
    align-self:     flex-end;
  }
  .composer-input::placeholder {
    color: var(--color-fg-subtle);
  }
  .composer-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Buttons ─────────────────────────────────────────────────────────────── */
  .composer-btn,
  .composer-send {
    display:        flex;
    align-items:    center;
    justify-content: center;
    width:          var(--hit-sm);
    height:         var(--hit-sm);
    border:         none;
    border-radius:  var(--radius-sm);
    cursor:         pointer;
    flex-shrink:    0;
    transition:     background var(--duration-fast) var(--ease-standard);  /* [feedback] */
    outline:        none;
  }
  .composer-btn:focus-visible,
  .composer-send:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* Attach */
  .composer-btn {
    background:  transparent;
    color:       var(--color-fg-subtle);
  }
  .composer-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color:      var(--color-fg-muted);
  }
  .composer-btn:disabled {
    opacity: 0.4;
    cursor:  not-allowed;
  }

  /* Send */
  .composer-send {
    background:  var(--color-accent);
    color:       var(--color-on-accent);
  }
  .composer-send:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }
  .composer-send:disabled {
    background: var(--color-bg-hover);
    color:      var(--color-fg-subtle);
    cursor:     not-allowed;
  }

  /* ── Send rebound [feedback] — scale spring-snap 0.93→1.02→1 ───────────── */
  .composer-row-rebound {
    animation: send-rebound 180ms var(--ease-emphasized-decelerate, cubic-bezier(0.05, 0.7, 0.1, 1)) both;
    transform-origin: center bottom;
  }
  @keyframes send-rebound {
    0%   { transform: scale(1);    }
    30%  { transform: scale(0.93); }   /* press trough */
    72%  { transform: scale(1.02); }   /* spring overshoot */
    100% { transform: scale(1);    }
  }

  @media (prefers-reduced-motion: reduce) {
    .composer-row-rebound { animation: none !important; }
  }
</style>
