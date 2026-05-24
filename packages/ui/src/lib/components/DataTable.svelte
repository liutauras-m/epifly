<svelte:options runes={true} />
<script lang="ts" generics="T extends Record<string, unknown>">
  /**
   * DataTable — sortable data table with responsive mobile-card layout (Phase 4.5).
   *
   * Under 768 px it renders each row as a card (key–value pairs); above 768 px it
   * renders a standard <table> with sticky header.
   *
   * Usage:
   *   <DataTable
   *     columns={[{ key: 'date', label: 'Date', sortable: true }, ...]}
   *     rows={invoices}
   *   >
   *     {#snippet cell({ key, value, row })}
   *       {#if key === 'status'}
   *         <StatusBadge status={value} label={value} />
   *       {:else}
   *         {value}
   *       {/if}
   *     {/snippet}
   *   </DataTable>
   *
   * Accessibility
   *   - <table role="grid"> with <caption> for screen readers
   *   - Sort buttons announce direction via aria-sort on <th>
   *   - Card layout has role="list" with each card as role="listitem"
   */
  import type { Snippet } from 'svelte';
  import type { Column } from './DataTable.types.js';

  type SortDir = 'asc' | 'desc' | null;

  let {
    columns,
    rows,
    caption,
    emptyMessage = 'No data.',
    class: cls = '',
    cell,
  }: {
    columns:       Column[];
    rows:          T[];
    caption?:      string;
    emptyMessage?: string;
    class?:        string;
    cell?:         Snippet<[{ key: string; value: unknown; row: T; colIndex: number }]>;
  } = $props();

  let sortKey = $state<string | null>(null);
  let sortDir = $state<SortDir>(null);

  function toggleSort(col: Column) {
    if (!col.sortable) return;
    if (sortKey !== col.key) {
      sortKey = col.key;
      sortDir = 'asc';
    } else if (sortDir === 'asc') {
      sortDir = 'desc';
    } else {
      sortKey = null;
      sortDir = null;
    }
  }

  const sortedRows = $derived.by(() => {
    if (!sortKey || !sortDir) return rows;
    return [...rows].sort((a, b) => {
      const av = a[sortKey!];
      const bv = b[sortKey!];
      const cmp =
        av === bv ? 0 :
        av == null ? 1 :
        bv == null ? -1 :
        String(av).localeCompare(String(bv), undefined, { numeric: true });
      return sortDir === 'asc' ? cmp : -cmp;
    });
  });

  function ariaSortFor(col: Column): 'ascending' | 'descending' | 'none' | undefined {
    if (!col.sortable) return undefined;
    if (sortKey !== col.key || !sortDir) return 'none';
    return sortDir === 'asc' ? 'ascending' : 'descending';
  }
</script>

<div class="data-table-wrap{cls ? ` ${cls}` : ''}">
  <!-- ── Desktop table (≥ 768px) ─────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- scrollable region per WCAG 2.1.1 SC -->
  <div class="table-shell" role="region" aria-label={caption ?? 'Data table'} tabindex="0">
    <table class="table" aria-label={caption}>
      {#if caption}
        <caption class="table-caption">{caption}</caption>
      {/if}
      <thead class="table-head">
        <tr>
          {#each columns as col (col.key)}
            <th
              class="th th-{col.align ?? 'left'}"
              scope="col"
              aria-sort={ariaSortFor(col)}
            >
              {#if col.sortable}
                <button
                  class="sort-btn"
                  type="button"
                  onclick={() => toggleSort(col)}
                  aria-label="Sort by {col.label}{sortKey === col.key && sortDir === 'asc' ? ', ascending' : sortKey === col.key && sortDir === 'desc' ? ', descending' : ''}"
                >
                  {col.label}
                  <span class="sort-icon" aria-hidden="true">
                    {sortKey === col.key && sortDir === 'asc'  ? '↑' :
                     sortKey === col.key && sortDir === 'desc' ? '↓' : '⇅'}
                  </span>
                </button>
              {:else}
                {col.label}
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody class="table-body">
        {#if sortedRows.length === 0}
          <tr>
            <td class="empty-cell" colspan={columns.length}>{emptyMessage}</td>
          </tr>
        {:else}
          {#each sortedRows as row, rowIdx (rowIdx)}
            <tr class="tr">
              {#each columns as col, colIdx (col.key)}
                <td class="td td-{col.align ?? 'left'}">
                  {#if cell}
                    {@render cell({ key: col.key, value: row[col.key], row, colIndex: colIdx })}
                  {:else}
                    {row[col.key] ?? '—'}
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>

  <!-- ── Mobile cards (< 768px) ──────────────────────────────────────────── -->
  <ul class="card-list" role="list">
    {#if sortedRows.length === 0}
      <li class="card-empty">{emptyMessage}</li>
    {:else}
      {#each sortedRows as row, rowIdx (rowIdx)}
        <li class="card" role="listitem">
          {#each columns as col, colIdx (col.key)}
            <div class="card-row">
              <span class="card-key">{col.cardLabel ?? col.label}</span>
              <span class="card-val">
                {#if cell}
                  {@render cell({ key: col.key, value: row[col.key], row, colIndex: colIdx })}
                {:else}
                  {row[col.key] ?? '—'}
                {/if}
              </span>
            </div>
          {/each}
        </li>
      {/each}
    {/if}
  </ul>
</div>

<style>
  .data-table-wrap {
    width:          100%;
    container-type: inline-size;
    container-name: data-table;
  }

  /* ── Desktop table ── */
  .table-shell {
    overflow-x: auto;
    border:     1px solid var(--color-border);
    border-radius: var(--radius-lg);
    outline:    none;
  }
  .table-shell:focus-visible {
    outline: var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .table {
    width:           100%;
    border-collapse: collapse;
    font-family:     var(--font-family-sans);
    font-size:       var(--font-size-meta);
    color:           var(--color-fg);
  }

  .table-caption {
    text-align:  left;
    font-size:   var(--font-size-label);
    font-weight: 600;
    color:       var(--color-fg-subtle);
    padding:     var(--space-3) var(--space-4) 0;
    caption-side: top;
  }

  .table-head { background: var(--color-bg-raised); }

  .th {
    padding:     var(--space-3) var(--space-4);
    font-family: var(--font-family-mono);
    font-size:   var(--font-size-label);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color:       var(--color-fg-subtle);
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
    position:    sticky;
    top:         0;
    background:  var(--color-bg-raised);
    z-index:     1;
  }

  .th-left   { text-align: left; }
  .th-center { text-align: center; }
  .th-right  { text-align: right; }

  .sort-btn {
    display:     inline-flex;
    align-items: center;
    gap:         var(--space-1);
    background:  none;
    border:      none;
    padding:     0;
    font:        inherit;
    color:       inherit;
    cursor:      pointer;
    white-space: nowrap;
    letter-spacing: inherit;
    text-transform: inherit;
  }
  .sort-btn:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
    border-radius:  var(--radius-xs);
  }

  .sort-icon {
    font-size:   var(--font-size-label);
    color:       var(--color-fg-subtle);
    opacity:     0.6;
  }

  .table-body tr:not(:last-child) .td {
    border-bottom: 1px solid var(--color-border);
  }

  .tr:hover { background: var(--color-bg-hover); }

  .td {
    padding: var(--space-3) var(--space-4);
    vertical-align: middle;
    color:   var(--color-fg);
  }

  .td-left   { text-align: left; }
  .td-center { text-align: center; }
  .td-right  { text-align: right; }

  .empty-cell {
    padding:    var(--space-6) var(--space-4);
    text-align: center;
    color:      var(--color-fg-subtle);
    font-style: italic;
  }

  /* ── Mobile cards (hidden on ≥ 768px via responsive show/hide) ── */
  .card-list {
    list-style: none;
    margin:     0;
    padding:    0;
    display:    flex;
    flex-direction: column;
    gap:        var(--space-2);
  }

  .card {
    border:        1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background:    var(--color-bg-raised);
    overflow:      hidden;
  }

  .card-row {
    display:       flex;
    justify-content: space-between;
    align-items:   flex-start;
    gap:           var(--space-3);
    padding:       var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  .card-row:last-child { border-bottom: none; }

  .card-key {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    font-weight:    600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color:          var(--color-fg-subtle);
    white-space:    nowrap;
    flex-shrink:    0;
  }

  .card-val {
    font-size:   var(--font-size-meta);
    color:       var(--color-fg);
    text-align:  right;
  }

  .card-empty {
    padding:    var(--space-5) var(--space-4);
    text-align: center;
    color:      var(--color-fg-subtle);
    font-style: italic;
    font-size:  var(--font-size-meta);
  }

  /* ── Responsive show/hide via container query ── */
  .table-shell { display: none; }
  .card-list   { display: flex; }

  @container data-table (min-width: 540px) {
    .table-shell { display: block; }
    .card-list   { display: none; }
  }
</style>
