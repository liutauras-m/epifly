<svelte:options runes={true} />
<script lang="ts">
  /**
   * QuotaList — capability-quota overview list (Phase 4.6).
   *
   * Renders a list of capabilities with their usage progress bars.
   * Composes CapabilityRow (name / kind / description) with UsageMeter
   * (progress bar) into a scannable quota overview.
   *
   * Usage:
   *   <QuotaList items={quotas} />
   *
   * Placement: features/ — "quota" and "capability" are both app-domain nouns
   * (Principle #10 classification table). The generic UsageMeter lives in components/.
   */
  import UsageMeter from '../components/UsageMeter.svelte';

  export interface QuotaItem {
    /** Capability ID or name key (used as list key). */
    id:          string;
    /** Display name for the capability. */
    name:        string;
    /** Short kind label (e.g. "hosted", "remote", "local"). */
    kind?:       string;
    /** Optional description shown as secondary text. */
    description?: string;
    /** Current usage count. */
    used:        number;
    /** Maximum allowed usage (null = unlimited). */
    limit:       number | null;
    /** Unit label (default "turns"). */
    unit?:       string;
  }

  let {
    items,
    emptyMessage = 'No capability quotas.',
  }: {
    items:         QuotaItem[];
    emptyMessage?: string;
  } = $props();

  function usageLabel(item: QuotaItem): string {
    const unit = item.unit ?? 'turns';
    const used = item.used.toLocaleString();
    if (!item.limit) return `${used} ${unit}`;
    return `${used} / ${item.limit.toLocaleString()} ${unit}`;
  }
</script>

{#if items.length === 0}
  <p class="empty">{emptyMessage}</p>
{:else}
  <ul class="quota-list" role="list">
    {#each items as item (item.id)}
      <li class="quota-item" role="listitem">
        <div class="quota-header">
          <div class="quota-meta">
            <span class="quota-name">{item.name}</span>
            {#if item.kind}
              <span class="quota-kind">{item.kind}</span>
            {/if}
          </div>
          <span class="quota-value" aria-label={usageLabel(item)}>
            {usageLabel(item)}
          </span>
        </div>
        {#if item.description}
          <p class="quota-desc">{item.description}</p>
        {/if}
        {#if item.limit}
          <UsageMeter
            label=""
            used={item.used}
            limit={item.limit}
            unit={item.unit ? ` ${item.unit}` : ''}
          />
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .empty {
    color:       var(--color-fg-subtle);
    font-size:   var(--font-size-meta);
    font-style:  italic;
    margin:      var(--space-5) 0;
    text-align:  center;
  }

  .quota-list {
    list-style: none;
    margin:     0;
    padding:    0;
    display:    flex;
    flex-direction: column;
    gap:        0;
    border:     1px solid var(--color-border);
    border-radius: var(--radius-lg);
    overflow:   hidden;
  }

  .quota-item {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-2);
    padding:        var(--space-4);
    background:     var(--color-bg-raised);
    border-bottom:  1px solid var(--color-border);
  }
  .quota-item:last-child { border-bottom: none; }

  .quota-header {
    display:       flex;
    align-items:   center;
    justify-content: space-between;
    gap:           var(--space-3);
  }

  .quota-meta {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
    min-width:   0;
    flex:        1;
  }

  .quota-name {
    font-size:   var(--font-size-meta);
    font-weight: 550;
    color:       var(--color-fg);
    overflow:    hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quota-kind {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    font-weight:    600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color:          var(--color-fg-subtle);
    background:     var(--color-bg-hover);
    border:         1px solid var(--color-border);
    border-radius:  var(--radius-xs);
    padding:        var(--space-half) var(--space-1);
    white-space:    nowrap;
    flex-shrink:    0;
  }

  .quota-value {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-meta);
    color:          var(--color-fg-muted);
    white-space:    nowrap;
    flex-shrink:    0;
  }

  .quota-desc {
    margin:      0;
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-subtle);
    line-height: 1.4;
  }
</style>
