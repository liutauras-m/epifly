<script lang="ts">
  /**
   * Hosted-project card — shown in the chat after a successful `host_project`
   * tool call that returned a `public_url`.
   *
   * Receives the live URL, optional project path, and optional framework name.
   * Renders a compact, branded card with a prominent "Open app" CTA.
   */

  let {
    url,
    projectPath = '',
    framework = '',
  }: {
    /** The live public URL returned by the host_project tool. */
    url: string;
    /** Workspace-relative project path, e.g. "projects/demo-app". */
    projectPath?: string;
    /** Detected framework name, e.g. "sveltekit", "vite-react". */
    framework?: string;
  } = $props();

  const displayPath = $derived(projectPath || url.split('/').at(-2) || 'project');

  const frameworkLabel = $derived(
    framework
      ? framework
          .replace('vite-', '')
          .replace('nextjs', 'Next.js')
          .replace('nuxt', 'Nuxt')
          .replace('sveltekit', 'SvelteKit')
          .replace('react', 'React')
          .replace('svelte', 'Svelte')
      : ''
  );
</script>

<div class="hosted-card" role="article" aria-label="Hosted project">
  <!-- Live indicator + label -->
  <div class="card-header">
    <span class="live-badge" aria-label="Live">
      <span class="live-dot" aria-hidden="true"></span>
      Live
    </span>
    {#if frameworkLabel}
      <span class="fw-badge">{frameworkLabel}</span>
    {/if}
    <span class="path" title={displayPath}>{displayPath}</span>
  </div>

  <!-- URL preview + CTA -->
  <div class="card-body">
    <span class="url-preview" title={url}>{url}</span>
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      class="open-btn"
      aria-label="Open hosted app in new tab"
    >
      <svg class="btn-icon" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M6 3H3a1 1 0 00-1 1v9a1 1 0 001 1h9a1 1 0 001-1v-3M9 2h5m0 0v5m0-5L7.5 8.5"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      Open app
    </a>
  </div>
</div>

<style>
  .hosted-card {
    border: 1px solid var(--rule);
    border-radius: var(--radius-md, 10px);
    overflow: hidden;
    margin: var(--space-2, 8px) 0;
    background: var(--paper, #fff);
    transition: box-shadow 160ms var(--ease-out, ease);
    max-width: 480px;
  }

  .hosted-card:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
  }

  /* ── Header ── */
  .card-header {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: var(--paper-2, #f5f5f5);
    border-bottom: 1px solid var(--rule);
    font-size: var(--font-size-meta, 0.75rem);
  }

  .live-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--success-soft, rgba(34, 160, 96, 0.12));
    color: var(--success, #1a7f4b);
    border-radius: 99px;
    padding: 2px 8px 2px 6px;
    font-weight: 600;
    font-size: 0.7rem;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .live-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success, #1a7f4b);
    animation: live-pulse 2.4s ease-in-out infinite;
    flex-shrink: 0;
  }

  @keyframes live-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.4; transform: scale(0.75); }
  }

  .fw-badge {
    background: var(--cyan-soft, rgba(0, 212, 255, 0.1));
    color: var(--cyan, #00D4FF);
    border-radius: 99px;
    padding: 1px 7px;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .path {
    color: var(--ink-3, #888);
    font-family: var(--font-mono, monospace);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  /* ── Body ── */
  .card-body {
    display: flex;
    align-items: center;
    gap: var(--space-3, 12px);
    padding: var(--space-3, 12px);
  }

  .url-preview {
    flex: 1;
    font-family: var(--font-mono, monospace);
    font-size: var(--font-size-meta, 0.75rem);
    color: var(--ink-2, #555);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .open-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1, 4px);
    background: var(--ember, #FF6200);
    color: #fff;
    border-radius: var(--radius-sm, 6px);
    padding: 6px 14px;
    font-size: var(--font-size-meta, 0.75rem);
    font-weight: 600;
    text-decoration: none;
    white-space: nowrap;
    flex-shrink: 0;
    transition: background 120ms var(--ease-out, ease), transform 80ms ease;
  }

  .open-btn:hover  { background: var(--ember-2, #E05500); }
  .open-btn:active { transform: scale(0.97); }
  .open-btn:focus-visible {
    outline: 2px solid var(--ember, #FF6200);
    outline-offset: 2px;
  }

  .btn-icon {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
  }

  /* ── Reduced-motion ── */
  @media (prefers-reduced-motion: reduce) {
    .live-dot    { animation: none; }
    .open-btn    { transition: none; }
    .hosted-card { transition: none; }
  }
</style>
