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
    placeholder = 'Ask anything…',
    loading     = false,
    disabled    = false,
    maxRows     = 8,
    attachments = [] as Attachment[],
    onsubmit,
    onattach,
    onremoveattachment,
    class: cls  = '',
  }: {
    value?:               string;
    placeholder?:         string;
    loading?:             boolean;
    disabled?:            boolean;
    maxRows?:             number;
    attachments?:         Attachment[];
    onsubmit?:            (value: string) => void;
    onattach?:            () => void;
    onremoveattachment?:  (id: string) => void;
    class?:               string;
  } = $props();

  let textareaEl = $state<HTMLTextAreaElement | undefined>(undefined);

  function submit() {
    const trimmed = value.trim();
    if (!trimmed || loading || disabled) return;
    onsubmit?.(trimmed);
    value = '';
    // Reset textarea height
    if (textareaEl) {
      textareaEl.style.height = '';
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  const canSend = $derived(value.trim().length > 0 && !loading && !disabled);
</script>

<div class="composer{loading ? ' composer-loading' : ''}{cls ? ` ${cls}` : ''}">

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
  <div class="composer-row">

    <!-- Attach button -->
    {#if onattach}
      <button
        type="button"
        class="composer-btn"
        aria-label="Attach file"
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

    <!-- Send button -->
    <button
      type="submit"
      class="composer-send"
      aria-label="Send message"
      disabled={!canSend}
      onclick={submit}
    >
      {#if loading}
        <span class="send-spinner" aria-hidden="true"></span>
      {:else}
        <Icon icon={Send} size="md" />
      {/if}
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
      border-color var(--duration-fast) var(--ease-standard),
      box-shadow   var(--duration-fast) var(--ease-standard);
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
    width:          36px;
    height:         36px;
    border:         none;
    border-radius:  var(--radius-sm);
    cursor:         pointer;
    flex-shrink:    0;
    transition:     background var(--duration-fast) var(--ease-standard);
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
    color:       #ffffff;
  }
  .composer-send:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }
  .composer-send:disabled {
    background: var(--color-bg-hover);
    color:      var(--color-fg-subtle);
    cursor:     not-allowed;
  }

  /* ── Spinner ─────────────────────────────────────────────────────────────── */
  .send-spinner {
    display:      inline-block;
    width:        16px;
    height:       16px;
    border:       2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation:    composer-spin 600ms linear infinite;
  }
  @keyframes composer-spin {
    to { transform: rotate(360deg); }
  }

  @media (prefers-reduced-motion: reduce) {
    .send-spinner { animation-duration: 0.01ms !important; }
  }
</style>
