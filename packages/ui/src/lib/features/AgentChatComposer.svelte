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
    onUpload?: (files: File[]) => Promise<void>;
  } = $props();

  let composerFocused = $state(false);
  let dropTarget = $state(false);

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
  }

  async function handleFiles(files: File[]) {
    await onUpload?.(files);
  }
</script>

<div class="composer-wrap">
  <form class="composer"
    class:drop-target={dropTarget}
    class:focused={composerFocused}
    class:has-content={value.length > 0 || attachments.length > 0}
    aria-busy={inFlight}
    onsubmit={handleSubmit}
    onfocusin={() => (composerFocused = true)}
    onfocusout={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) composerFocused = false; }}
    ondragover={(e) => { if (e.dataTransfer?.types?.includes('Files')) { e.preventDefault(); dropTarget = true; } }}
    ondragleave={() => (dropTarget = false)}
    ondrop={(e) => { e.preventDefault(); dropTarget = false; if (e.dataTransfer?.files?.length) handleFiles([...e.dataTransfer.files]); }}>

    {#if attachments.length}
      <div class="attachments">
        {#each attachments as a (a.id)}
          <span class="attachment">
            <svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 2h7l4 4v11H4z"/><polyline points="11,2 11,6 15,6"/></svg>
            <span class="attachment-name">{a.filename}</span>
            <span class="attachment-size">{fmtSize(a.size)}</span>
            <button type="button" class="attachment-remove" aria-label="Remove {a.filename}"
              onclick={() => (attachments = attachments.filter(x => x.id !== a.id))}>
              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="10" height="10">
                <line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
              </svg>
            </button>
          </span>
        {/each}
      </div>
    {/if}

    <label class="sr-only" for="agent-prompt">Message</label>
    <textarea id="agent-prompt" class="composer-input" name="prompt"
      placeholder="How can I help you today?" rows="1"
      autocomplete="off" bind:value
      use:autoGrow
      onkeydown={(e) => {
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          (e.currentTarget.closest('form') as HTMLFormElement)?.requestSubmit();
        }
      }}></textarea>

    <input id="composer-file-input" type="file" style="display:none" multiple
      onchange={(e) => { const f = e.currentTarget.files; if (f?.length) handleFiles([...f]); e.currentTarget.value = ''; }}>

    <div class="composer-toolbar">
      <button type="button" class="toolbar-btn" aria-label="Attach file"
        onclick={() => document.getElementById('composer-file-input')?.click()}>
        <svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M15 9l-6 6a4 4 0 0 1-5.657-5.657l7-7a2.5 2.5 0 0 1 3.536 3.536l-7 7a1 1 0 0 1-1.414-1.414l6-6"/>
        </svg>
      </button>
      <div class="toolbar-spacer"></div>
      <button type="submit" class="send-btn" aria-label="Send message" disabled={inFlight}>
        <svg class="icon" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round">
          <line x1="7" y1="12" x2="7" y2="2"/><polyline points="3,6 7,2 11,6"/>
        </svg>
      </button>
    </div>
  </form>
</div>

<style>
  .sr-only { position:absolute;width:1px;height:1px;padding:0;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0; }
  .composer-wrap { width: 100%; max-width: var(--composer-w, 720px); margin: 0 auto; padding: 0 var(--s-4); }
  .composer {
    position: relative; border: 1px solid var(--rule); border-radius: var(--r-md);
    background: var(--paper-2); transition: border-color var(--dur-1), box-shadow var(--dur-1);
    display: flex; flex-direction: column;
  }
  .composer.focused { border-color: var(--ember); box-shadow: 0 0 0 3px var(--ember-soft); }
  .composer.drop-target { border-color: var(--ember); background: var(--ember-soft); }
  .attachments {
    display: flex; flex-wrap: wrap; gap: var(--s-2);
    padding: var(--s-2) var(--s-3) 0;
  }
  .attachment {
    display: inline-flex; align-items: center; gap: var(--s-1);
    background: var(--paper-3); border-radius: var(--r-xs);
    padding: 2px var(--s-2); font-size: var(--t-label); font-family: var(--font-mono);
  }
  .attachment-name { max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .attachment-size { color: var(--ink-3); }
  .attachment-remove { background: none; border: none; padding: 0; cursor: pointer; color: var(--ink-3); }
  .attachment-remove:hover { color: var(--danger); }
  .composer-input {
    width: 100%; padding: var(--s-3) var(--s-4);
    background: transparent; border: none; outline: none; resize: none;
    font-family: var(--font-body); font-size: var(--t-body); color: var(--ink);
    line-height: 1.55; min-height: 44px; max-height: 240px;
  }
  .composer-input::placeholder { color: var(--ink-3); }
  .composer-toolbar {
    display: flex; align-items: center; padding: var(--s-2) var(--s-3);
    border-top: 1px solid var(--rule);
  }
  .toolbar-spacer { flex: 1; }
  .toolbar-btn, .send-btn {
    display: flex; align-items: center; justify-content: center;
    background: none; border: none; cursor: pointer;
    width: 32px; height: 32px; border-radius: var(--r-sm);
    color: var(--ink-3); transition: color var(--dur-1), background var(--dur-1);
  }
  .toolbar-btn:hover { background: var(--paper-3); color: var(--ink); }
  .send-btn { background: var(--ember); color: var(--paper); border-radius: var(--r-sm); }
  .send-btn:hover:not(:disabled) { background: var(--ember-2); }
  .send-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .icon { width: 16px; height: 16px; }
</style>
