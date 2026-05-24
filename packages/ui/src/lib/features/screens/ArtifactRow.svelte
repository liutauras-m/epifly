<svelte:options runes={true} />
<script lang="ts">
  /**
   * ArtifactRow — Phase 4.8
   * Artifact card used in the ArtifactsScreen grid. Works as both a compact
   * list row (narrow containers) and a card tile (wide containers).
   */
  let {
    name,
    size,
    updatedAt,
    selected = false,
    onClick,
  }: {
    name: string;
    size?: number;
    updatedAt?: string;
    selected?: boolean;
    onClick: () => void;
  } = $props();

  function fmtSize(n: number) {
    if (n < 1024) return `${n} B`;
    if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1048576).toFixed(1)} MB`;
  }

  function fmtDate(ts: string) {
    return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  /** File extension → icon accent colour bucket */
  const ext = $derived(name.split('.').pop()?.toLowerCase() ?? '');
</script>

<button
  class="artifact-card"
  class:selected
  onclick={onClick}
  aria-pressed={selected}
>
  <!-- File icon badge -->
  <div class="file-badge" data-ext={ext}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
      stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
      width="24" height="24" aria-hidden="true">
      <path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
      <polyline points="13 2 13 9 20 9"/>
    </svg>
    {#if ext}
      <span class="ext-label">{ext}</span>
    {/if}
  </div>

  <div class="artifact-info">
    <span class="artifact-name">{name}</span>
    <span class="artifact-meta">
      {#if size != null}{fmtSize(size)}{/if}
      {#if updatedAt} · {fmtDate(updatedAt)}{/if}
    </span>
  </div>

  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
    stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
    width="16" height="16" class="row-arrow" aria-hidden="true">
    <path d="M9 18l6-6-6-6"/>
  </svg>
</button>

<style>
  .artifact-card {
    display:          flex;
    align-items:      center;
    gap:              var(--space-3);
    padding:          var(--space-3) var(--space-4);
    border:           1px solid var(--color-border);
    border-radius:    var(--radius-md);
    background:       var(--color-bg-raised);
    cursor:           pointer;
    width:            100%;
    text-align:       left;
    min-height:       var(--hit, 44px);
    transition:       background var(--duration-fast) var(--ease-standard),
                      border-color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .artifact-card:hover {
    background:     var(--color-bg-hover);
    border-color:   var(--color-border-hover, var(--color-border));
  }

  .artifact-card:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .artifact-card.selected {
    border-color:   var(--color-accent);
    background:     var(--color-accent-soft, var(--color-bg-raised));
  }

  /* File icon badge */
  .file-badge {
    position:       relative;
    flex-shrink:    0;
    color:          var(--color-fg-subtle);
    display:        flex;
    align-items:    center;
    justify-content: center;
  }

  .ext-label {
    position:       absolute;
    bottom:         -2px;
    left:           50%;
    transform:      translateX(-50%);
    font-size:      8px;
    font-family:    var(--font-family-mono);
    font-weight:    700;
    color:          var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.02em;
    line-height:    1;
    max-width:      28px;
    overflow:       hidden;
    text-overflow:  clip;
    white-space:    nowrap;
  }

  .artifact-info {
    flex:           1;
    display:        flex;
    flex-direction: column;
    gap:            2px;
    overflow:       hidden;
    min-width:      0;
  }

  .artifact-name {
    font-size:      var(--font-size-body);
    color:          var(--color-fg);
    overflow:       hidden;
    text-overflow:  ellipsis;
    white-space:    nowrap;
    font-weight:    450;
  }

  .artifact-meta {
    font-family:  var(--font-family-mono);
    font-size:    var(--font-size-label);
    color:        var(--color-fg-subtle);
  }

  .row-arrow { color: var(--color-fg-subtle); flex-shrink: 0; }

  @media (prefers-reduced-motion: reduce) {
    .artifact-card { transition: none; }
  }
</style>
