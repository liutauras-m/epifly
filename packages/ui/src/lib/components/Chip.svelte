<svelte:options runes={true} />
<script lang="ts">
  /**
   * Chip — filter / tag primitive (Phase 2.7).
   *
   * Two variants:
   *   tonal   — soft ember background when selected (default); muted when idle.
   *             Use for filter sets (capability browser, artifact tags, suggestion chips).
   *   outlined — border-only, no fill.
   *             Use for removable tags and compact metadata labels.
   *
   * Chips are either static (display-only) or interactive (button / checkbox).
   * Pass `onclick` or `onremove` to make them interactive.
   *
   * Usage:
   *   <Chip label="Python" />
   *   <Chip label="Stable Diffusion" selected />
   *   <Chip label="Beta" variant="outlined" />
   *   <Chip label="science" onremove={handleRemove} />
   */
  import type { Component } from 'svelte';
  import Icon from './Icon.svelte';
  import { X } from 'lucide-svelte';

  export type ChipVariant = 'tonal' | 'outlined';
  export type ChipSize    = 'sm' | 'md';

  let {
    label,
    variant   = 'tonal'  as ChipVariant,
    size      = 'md'     as ChipSize,
    selected  = false,
    disabled  = false,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon,
    class: cls = '',
    onclick,
    onremove,
    ...rest
  }: {
    label:        string;
    variant?:     ChipVariant;
    size?:        ChipSize;
    selected?:    boolean;
    disabled?:    boolean;
    /** Optional leading icon (lucide-svelte component) */
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon?:        Component<any>;
    class?:       string;
    onclick?:     (e: MouseEvent) => void;
    /** When provided the chip renders a ✕ remove button */
    onremove?:    (e: MouseEvent) => void;
    [key: string]: unknown;
  } = $props();

  const interactive  = $derived(Boolean(onclick));
  const removable    = $derived(Boolean(onremove));
  const iconSz       = $derived(size === 'sm' ? 'sm' : 'sm');  // chips always use sm icons
</script>

<!--
  Render as <button> if interactive, otherwise <span> (display only).
  The remove button is always a nested <button> regardless of parent.
-->
{#if interactive}
  <button
    type="button"
    class="chip chip-{variant} chip-{size}{selected ? ' chip-selected' : ''}{removable ? ' chip-removable' : ''}{cls ? ` ${cls}` : ''}"
    {disabled}
    aria-pressed={selected}
    {onclick}
    {...rest}
  >
    {#if icon}<Icon {icon} size={iconSz} />{/if}
    <span class="chip-label">{label}</span>
    {#if removable}
      <button
        type="button"
        class="chip-remove"
        tabindex="-1"
        aria-label="Remove {label}"
        onclick={(e) => { e.stopPropagation(); onremove?.(e); }}
      >
        <Icon icon={X} size="sm" />
      </button>
    {/if}
  </button>
{:else}
  <span
    class="chip chip-{variant} chip-{size}{selected ? ' chip-selected' : ''}{removable ? ' chip-removable' : ''}{cls ? ` ${cls}` : ''}"
    {...rest}
  >
    {#if icon}<Icon {icon} size={iconSz} />{/if}
    <span class="chip-label">{label}</span>
    {#if removable}
      <button
        type="button"
        class="chip-remove"
        aria-label="Remove {label}"
        onclick={onremove}
      >
        <Icon icon={X} size="sm" />
      </button>
    {/if}
  </span>
{/if}

<style>
  /* ── Base ────────────────────────────────────────────────────────────────── */
  .chip {
    display:        inline-flex;
    align-items:    center;
    gap:            var(--space-1);
    white-space:    nowrap;
    flex-shrink:    0;

    font-family:    var(--font-family-sans);
    font-weight:    450;

    border-radius:  var(--radius-full);
    border:         1px solid transparent;
    cursor:         default;

    transition:
      background   var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard),
      color        var(--duration-fast) var(--ease-standard);
  }

  /* Interactive chips */
  button.chip {
    cursor:  pointer;
    outline: none;
  }
  button.chip:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }
  button.chip:disabled {
    opacity: 0.45;
    cursor:  not-allowed;
  }

  /* ── Sizes ───────────────────────────────────────────────────────────────── */
  .chip-sm {
    height:    24px;
    padding:   0 var(--space-2);
    font-size: var(--font-size-label);    /* 11px */
    gap:       3px;
  }

  .chip-md {
    height:    32px;
    padding:   0 var(--space-3);
    font-size: var(--font-size-meta);     /* 13px */
  }

  /* ── Tonal ───────────────────────────────────────────────────────────────── */
  .chip-tonal {
    background:   var(--color-bg-raised);
    color:        var(--color-fg-muted);
    border-color: var(--color-border);
  }
  button.chip-tonal:hover:not(:disabled) {
    background:   var(--color-bg-hover);
    border-color: var(--color-border-strong);
  }
  .chip-tonal.chip-selected {
    background:   var(--color-accent-soft);
    color:        var(--color-accent);
    border-color: var(--color-accent);
  }
  button.chip-tonal.chip-selected:hover:not(:disabled) {
    background:   var(--color-accent-soft);
    filter:       brightness(0.95);
  }

  /* ── Outlined ────────────────────────────────────────────────────────────── */
  .chip-outlined {
    background:   transparent;
    color:        var(--color-fg-muted);
    border-color: var(--color-border);
  }
  button.chip-outlined:hover:not(:disabled) {
    background:   var(--color-bg-raised);
    border-color: var(--color-border-strong);
  }
  .chip-outlined.chip-selected {
    color:        var(--color-accent);
    border-color: var(--color-accent);
  }

  /* ── Label ───────────────────────────────────────────────────────────────── */
  .chip-label {
    line-height: 1;
  }

  /* ── Remove button ───────────────────────────────────────────────────────── */
  .chip-remove {
    display:        inline-flex;
    align-items:    center;
    justify-content: center;
    background:     transparent;
    border:         none;
    border-radius:  var(--radius-full);
    cursor:         pointer;
    padding:        0;
    color:          inherit;
    opacity:        0.6;
    line-height:    0;
    margin-left:    2px;
    transition:     opacity var(--duration-fast);
  }
  .chip-remove:hover { opacity: 1; }
</style>
