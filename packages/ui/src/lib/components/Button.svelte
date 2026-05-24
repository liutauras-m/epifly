<svelte:options runes={true} />
<script lang="ts">
  /**
   * Button — interactive primitive (Phase 2.7).
   *
   * Foundry's canonical button. All interactive CTAs in `apps/*` must use this
   * component — never a raw `<button>` with local CSS.
   *
   * Variants
   *   primary   — filled ember, main actions (login, save, send)
   *   secondary — tonal paper-3 background, secondary actions
   *   ghost     — borderless, tertiary / toolbar actions
   *   danger    — filled danger, destructive actions
   *   outline   — border-only, pairing with primary in button groups
   *
   * Sizes: sm (32 px), md (40 px — default), lg (48 px)
   *
   * Usage:
   *   <Button variant="primary" onclick={handleSave}>Save</Button>
   *   <Button variant="secondary" size="sm" loading={saving}>Saving…</Button>
   *   <Button variant="danger" disabled>Delete account</Button>
   */
  import type { Snippet } from 'svelte';
  import type { IconComponent } from './Icon.types.js';
  import Icon from './Icon.svelte';

  export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'outline';
  export type ButtonSize    = 'sm' | 'md' | 'lg';

  let {
    variant   = 'primary' as ButtonVariant,
    size      = 'md'      as ButtonSize,
    type      = 'button'  as 'button' | 'submit' | 'reset',
    loading   = false,
    disabled  = false,
    fullWidth = false,
    iconLeading,
    iconTrailing,
    class:    cls = '',
    children,
    text,
    onclick,
    ...rest
  }: {
    variant?:      ButtonVariant;
    size?:         ButtonSize;
    type?:         'button' | 'submit' | 'reset';
    loading?:      boolean;
    disabled?:     boolean;
    fullWidth?:    boolean;
    /** Lucide-svelte component to render before the label */
    iconLeading?:  IconComponent;
    /** Lucide-svelte component to render after the label */
    iconTrailing?: IconComponent;
    class?:        string;
    /** Convenience plain-text label (for fixture files) */
    text?:         string;
    children?:     Snippet;
    onclick?:      (e: MouseEvent) => void;
    [key: string]: unknown;
  } = $props();

  const iconSize = $derived(size === 'lg' ? 'md' : 'sm');
</script>

<button
  {type}
  class="btn btn-{variant} btn-{size}{fullWidth ? ' btn-full' : ''}{loading ? ' btn-loading' : ''}{cls ? ` ${cls}` : ''}"
  disabled={disabled || loading}
  aria-busy={loading || undefined}
  {onclick}
  {...rest}
>
  {#if iconLeading && !loading}
    <Icon icon={iconLeading} size={iconSize} />
  {/if}

  {#if loading}
    <span class="spinner" aria-hidden="true"></span>
  {/if}

  <span class="btn-label">
    {#if children}{@render children()}{:else}{text ?? ''}{/if}
  </span>

  {#if iconTrailing && !loading}
    <Icon icon={iconTrailing} size={iconSize} />
  {/if}
</button>

<style>
  /* ── Base ────────────────────────────────────────────────────────────────── */
  .btn {
    /* layout */
    display:        inline-flex;
    align-items:    center;
    justify-content: center;
    gap:            var(--space-2);
    white-space:    nowrap;
    flex-shrink:    0;

    /* typography */
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-meta);  /* 13px */
    font-weight:    500;
    letter-spacing: -0.01em;
    line-height:    1;

    /* box */
    border:         1px solid transparent;
    border-radius:  var(--radius-sm);
    cursor:         pointer;
    text-decoration: none;

    /* transitions [feedback] */
    transition:
      background     var(--duration-fast) var(--ease-standard),
      border-color   var(--duration-fast) var(--ease-standard),
      color          var(--duration-fast) var(--ease-standard),
      box-shadow     var(--duration-fast) var(--ease-standard),
      opacity        var(--duration-fast) var(--ease-standard),
      transform      var(--duration-fast) var(--ease-standard);

    /* focus ring */
    outline:        none;
  }

  .btn:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  .btn:disabled,
  .btn[disabled] {
    opacity: 0.45;
    cursor:  not-allowed;
    pointer-events: none;
  }

  .btn-full { width: 100%; }

  /* ── Sizes ───────────────────────────────────────────────────────────────── */
  .btn-sm { height: var(--chip-h-md); padding: 0 var(--space-3); border-radius: var(--radius-xs); }
  .btn-md { height: var(--hit-sm); padding: 0 var(--space-4); }
  .btn-lg { height: var(--topbar-height); padding: 0 var(--space-5); font-size: var(--font-size-body); }

  /* ── Variants ────────────────────────────────────────────────────────────── */

  /* primary — filled ember */
  .btn-primary {
    background:   var(--color-accent);
    color:        var(--color-on-accent);
    border-color: var(--color-accent);
  }
  .btn-primary:hover:not(:disabled) {
    background:   var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }
  .btn-primary:active:not(:disabled) {
    background:   var(--color-accent-hover);
    box-shadow:   inset 0 1px 3px rgba(0,0,0,0.18);
  }

  /* secondary — tonal */
  .btn-secondary {
    background:   var(--color-bg-hover);
    color:        var(--color-fg);
    border-color: var(--color-border);
  }
  .btn-secondary:hover:not(:disabled) {
    background:   var(--color-bg-raised);
    border-color: var(--color-border-strong);
  }

  /* ghost — no background */
  .btn-ghost {
    background:   transparent;
    color:        var(--color-fg-muted);
    border-color: transparent;
  }
  .btn-ghost:hover:not(:disabled) {
    background:   var(--color-bg-raised);
    color:        var(--color-fg);
  }

  /* danger — filled red */
  .btn-danger {
    background:   var(--color-danger);
    color:        var(--color-on-danger);
    border-color: var(--color-danger);
  }
  .btn-danger:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  /* outline — border only */
  .btn-outline {
    background:   transparent;
    color:        var(--color-accent);
    border-color: var(--color-accent);
  }
  .btn-outline:hover:not(:disabled) {
    background:   var(--color-accent-soft);
  }

  /* ── Loading spinner ─────────────────────────────────────────────────────── */
  .btn-loading .btn-label { opacity: 0.7; }

  .spinner {
    display:        inline-block;
    width:          var(--icon-xs);
    height:         var(--icon-xs);
    border:         2px solid currentColor;
    border-top-color: transparent;
    border-radius:  50%;
    animation:      btn-spin 600ms linear infinite;
    flex-shrink:    0;
  }
  @keyframes btn-spin {
    to { transform: rotate(360deg); }
  }

  /* ── Hover lift [feedback] — pointer-fine devices only (not touch) ──────── */
  @media (hover: hover) and (pointer: fine) {
    .btn:not(:disabled):hover {
      transform: translateY(-1px);
    }
    .btn:not(:disabled):active {
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation-duration: 0.01ms !important; }
    /* [feedback] reduced-motion: keep color/background only, drop transform */
    .btn { transition: background var(--duration-fast) var(--ease-standard),
                       border-color var(--duration-fast) var(--ease-standard),
                       color var(--duration-fast) var(--ease-standard) !important; }
  }
</style>
