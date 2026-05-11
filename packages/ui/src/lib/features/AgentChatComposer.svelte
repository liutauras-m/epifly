<script lang="ts">
  import { autoGrow } from '../utils/actions.js';

  export interface Attachment {
    id: string;
    filename: string;
    size: number;
  }

  let {
    value = $bindable(''),
    attachments = $bindable<Attachment[]>([]),
    inFlight = false,
    onsubmit,
    onUpload,
  }: {
    value?: string;
    attachments?: Attachment[];
    inFlight?: boolean;
    onsubmit: (prompt: string, attachments: Attachment[]) => void;
    onUpload?: (files: File[]) => Promise<Attachment[]>;
  } = $props();

  let composerFocused = $state(false);
  let dropTarget = $state(false);

  const hasContent = $derived(value.trim().length > 0 || attachments.length > 0);

  function fmtSize(n: number) {
    if (n < 1024) return `${n}B`;
    if (n < 1048576) return `${(n / 1024).toFixed(1)}KB`;
    return `${(n / 1048576).toFixed(1)}MB`;
  }

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const val = value.trim();
    if (!val && attachments.length === 0) return;
    onsubmit(val, attachments);
    value = '';
    attachments = [];
  }

  async function handleFiles(files: File[]) {
    const added = await onUpload?.(files);
    if (added?.length) attachments = [...attachments, ...added];
  }
</script>

<form
  class="composer"
  class:drop-target={dropTarget}
  class:focused={composerFocused}
  class:has-content={hasContent}
  aria-busy={inFlight}
  onsubmit={handleSubmit}
  onfocusin={() => (composerFocused = true)}
  onfocusout={(e) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node)) composerFocused = false;
  }}
  ondragover={(e) => {
    if (e.dataTransfer?.types?.includes('Files')) { e.preventDefault(); dropTarget = true; }
  }}
  ondragleave={() => (dropTarget = false)}
  ondrop={(e) => {
    e.preventDefault();
    dropTarget = false;
    if (e.dataTransfer?.files?.length) handleFiles([...e.dataTransfer.files]);
  }}
>
  {#if attachments.length}
    <div class="attachments">
      {#each attachments as a (a.id)}
        <span class="attachment">
          <svg class="attach-icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M4 2h7l4 4v11H4z"/><polyline points="11,2 11,6 15,6"/>
          </svg>
          <span class="attachment-name">{a.filename}</span>
          <span class="attachment-size">{fmtSize(a.size)}</span>
          <button
            type="button"
            class="attachment-remove"
            aria-label="Remove {a.filename}"
            onclick={() => (attachments = attachments.filter(x => x.id !== a.id))}
          >
            <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="10" height="10">
              <line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
            </svg>
          </button>
        </span>
      {/each}
    </div>
  {/if}

  <div class="composer-row">
    <input
      id="composer-file-input"
      type="file"
      style="display:none"
      multiple
      onchange={(e) => {
        const f = e.currentTarget.files;
        if (f?.length) handleFiles([...f]);
        e.currentTarget.value = '';
      }}
    />
    <button
      type="button"
      class="icon-btn attach-btn"
      aria-label="Attach file"
      onclick={() => document.getElementById('composer-file-input')?.click()}
    >
      <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="20" height="20">
        <path d="M15 9l-6 6a4 4 0 0 1-5.657-5.657l7-7a2.5 2.5 0 0 1 3.536 3.536l-7 7a1 1 0 0 1-1.414-1.414l6-6"/>
      </svg>
    </button>

    <label class="sr-only" for="agent-prompt">Message</label>
    <textarea
      id="agent-prompt"
      class="composer-input"
      name="prompt"
      placeholder="Message"
      rows="1"
      autocomplete="off"
      bind:value
      use:autoGrow
      onkeydown={(e) => {
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          (e.currentTarget.closest('form') as HTMLFormElement)?.requestSubmit();
        }
      }}
    ></textarea>

    <button
      type="submit"
      class="icon-btn send-btn"
      class:active={hasContent && !inFlight}
      aria-label="Send message"
      disabled={!hasContent || inFlight}
    >
      {#if inFlight}
        <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
          <rect x="4" y="4" width="8" height="8" rx="1.5"/>
        </svg>
      {:else}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" width="16" height="16">
          <line x1="8" y1="13" x2="8" y2="3"/><polyline points="4,7 8,3 12,7"/>
        </svg>
      {/if}
    </button>
  </div>
</form>

<style>
  .sr-only {
    position: absolute; width: 1px; height: 1px; padding: 0;
    overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0;
  }

  .composer {
    width: 100%;
    border: 1.5px solid var(--rule);
    border-radius: var(--r-lg);
    background: var(--paper-2);
    display: flex;
    flex-direction: column;
    transition: border-color var(--dur-1) var(--ease-out),
                box-shadow var(--dur-1) var(--ease-out);
  }

  .composer.focused {
    border-color: var(--ember);
    box-shadow: 0 0 0 3px var(--ember-soft);
  }

  .composer.drop-target {
    border-color: var(--ember);
    background: var(--ember-soft);
  }

  /* ── Attachments ──────────────────────────────── */
  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3) 0;
  }

  .attachment {
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    background: var(--paper-3);
    border-radius: var(--r-sm);
    padding: 3px var(--s-2);
    font-size: var(--t-meta);
    font-family: var(--font-mono);
  }

  .attach-icon { width: 14px; height: 14px; flex-shrink: 0; }
  .attachment-name { max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .attachment-size { color: var(--ink-3); }
  .attachment-remove { background: none; border: none; padding: 0; cursor: pointer; color: var(--ink-3); line-height: 0; }
  .attachment-remove:hover { color: var(--danger); }

  /* ── Inline row: attach | textarea | send ─────── */
  .composer-row {
    display: flex;
    flex-direction: row;
    align-items: flex-end;
    gap: 4px;
    padding: 6px 8px;
  }

  /* ── Textarea ─────────────────────────────────── */
  .composer-input {
    flex: 1;
    padding: 8px 4px;
    background: transparent;
    border: none;
    outline: none;
    resize: none;
    font-family: var(--font-body);
    font-size: 16px; /* prevents iOS zoom on focus */
    color: var(--ink);
    line-height: 1.55;
    min-height: 36px;
    max-height: 160px;
    box-sizing: border-box;
    -webkit-appearance: none;
    align-self: center;
  }

  .composer-input::placeholder { color: var(--ink-3); }

  /* ── Shared icon button base ──────────────────── */
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    flex-shrink: 0;
    transition: background var(--dur-1), color var(--dur-1), transform 0.1s;
    -webkit-tap-highlight-color: transparent;
  }

  /* ── Attach button ────────────────────────────── */
  .attach-btn {
    background: transparent;
    color: var(--ink-2);
  }

  .attach-btn:hover,
  .attach-btn:active {
    background: var(--paper-3);
    color: var(--ink);
  }

  /* ── Send button ──────────────────────────────── */
  .send-btn {
    background: var(--paper-3);
    color: var(--ink-3);
    border: none;
  }

  .send-btn.active {
    background: var(--ember);
    color: #fff;
  }

  .send-btn:disabled { cursor: default; }
  .send-btn:not(:disabled):active { transform: scale(0.88); }
</style>
