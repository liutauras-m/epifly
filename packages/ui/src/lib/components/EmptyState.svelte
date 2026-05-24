<svelte:options runes={true} />
<script lang="ts">
  /**
   * EmptyState — zero-data / error illustration (Phase 2.7).
   *
   * Rendered for every "no data" surface in the app:
   *   - No chats, no artifacts, no capabilities, no invoices
   *   - Route-level errors (wrapped by +error.svelte)
   *   - Permission-denied gates
   *
   * The illustration is a hairline-rule SVG drawn with `--ember` accent.
   * Kind maps to a unique glyph; adding kinds is cheap (extend the GLYPHS map).
   *
   * Usage:
   *   <EmptyState kind="no-chats" title="No conversations yet"
   *               body="Start a new chat to begin." action={newChat} actionLabel="New chat" />
   *
   *   <EmptyState kind="error" title="Something went wrong"
   *               body="Try refreshing. If the problem persists, contact support."
   *               action={reload} actionLabel="Refresh page" />
   */
  import type { Snippet } from 'svelte';
  import Button from './Button.svelte';
  import { RotateCcw, MessageSquarePlus, FolderOpen, Zap, FileText, Receipt, ShieldOff } from '@lucide/svelte';

  export type EmptyStateKind =
    | 'no-chats'
    | 'no-artifacts'
    | 'no-capabilities'
    | 'no-invoices'
    | 'error'
    | 'permission-denied'
    | 'generic';

  // Map kind → accent color token
  const KIND_COLOR: Record<EmptyStateKind, string> = {
    'no-chats':          'var(--color-accent)',
    'no-artifacts':      'var(--color-accent)',
    'no-capabilities':   'var(--color-accent)',
    'no-invoices':       'var(--color-accent)',
    'error':             'var(--color-danger)',
    'permission-denied': 'var(--color-danger)',
    'generic':           'var(--color-fg-subtle)',
  };

  let {
    kind          = 'generic' as EmptyStateKind,
    title,
    body,
    actionLabel,
    action,
    secondaryLabel,
    secondaryAction,
    compact       = false,
    children,
    class: cls    = '',
  }: {
    kind?:            EmptyStateKind;
    title:            string;
    body?:            string;
    actionLabel?:     string;
    action?:          () => void;
    secondaryLabel?:  string;
    secondaryAction?: () => void;
    /** Compact mode — smaller illustration, used inside cards and panels */
    compact?:         boolean;
    children?:        Snippet;
    class?:           string;
  } = $props();

  const accentColor = $derived(KIND_COLOR[kind]);
  const isDanger    = $derived(kind === 'error' || kind === 'permission-denied');
</script>

<div
  class="empty-state{compact ? ' compact' : ''}{cls ? ` ${cls}` : ''}"
  role="status"
  aria-live="polite"
>
  <!-- Hairline SVG illustration -->
  <div class="illustration" aria-hidden="true" style:--accent={accentColor}>
    {#if kind === 'no-chats'}
      <!-- Chat bubble silhouette -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect x="8"  y="8"  width="56" height="38" rx="8"  stroke="var(--accent)" stroke-width="1.5"/>
        <rect x="18" y="18" width="28" height="2"   rx="1"  fill="var(--accent)" opacity="0.5"/>
        <rect x="18" y="25" width="20" height="2"   rx="1"  fill="var(--accent)" opacity="0.35"/>
        <path d="M20 46 L16 58 L32 50" stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"/>
        <!-- Spark -->
        <circle cx="62" cy="14" r="4" stroke="var(--accent)" stroke-width="1.5"/>
        <path d="M60 14h4M62 12v4" stroke="var(--accent)" stroke-width="1" opacity="0.6"/>
      </svg>

    {:else if kind === 'no-artifacts'}
      <!-- Folder outline -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M10 22 C10 18 13 16 17 16 L30 16 L34 22 L66 22 C69 22 70 24 70 27 L70 54 C70 57 68 58 65 58 L15 58 C12 58 10 57 10 54 Z"
              stroke="var(--accent)" stroke-width="1.5"/>
        <rect x="28" y="36" width="24" height="2"  rx="1" fill="var(--accent)" opacity="0.4"/>
        <rect x="32" y="42" width="16" height="2"  rx="1" fill="var(--accent)" opacity="0.28"/>
      </svg>

    {:else if kind === 'no-capabilities'}
      <!-- Lightning bolt in circle -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <circle cx="40" cy="36" r="26" stroke="var(--accent)" stroke-width="1.5"/>
        <path d="M43 18 L34 38 L40 38 L37 54 L46 34 L40 34 Z"
              stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"/>
      </svg>

    {:else if kind === 'no-invoices'}
      <!-- Receipt outline -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M22 10 L22 62 L26 58 L30 62 L34 58 L38 62 L42 58 L46 62 L50 58 L54 62 L58 62 L58 10 Z"
              stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"/>
        <rect x="29" y="22" width="22" height="2" rx="1" fill="var(--accent)" opacity="0.5"/>
        <rect x="29" y="30" width="16" height="2" rx="1" fill="var(--accent)" opacity="0.35"/>
        <rect x="29" y="38" width="20" height="2" rx="1" fill="var(--accent)" opacity="0.35"/>
      </svg>

    {:else if kind === 'error'}
      <!-- Diamond / warning outline -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M40 10 L68 36 L40 62 L12 36 Z" stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"/>
        <rect x="38.5" y="24" width="3" height="16" rx="1.5" fill="var(--accent)"/>
        <circle cx="40" cy="47" r="2" fill="var(--accent)"/>
      </svg>

    {:else if kind === 'permission-denied'}
      <!-- Shield with lock -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M40 8 L64 18 L64 40 C64 52 53 62 40 64 C27 62 16 52 16 40 L16 18 Z"
              stroke="var(--accent)" stroke-width="1.5" stroke-linejoin="round"/>
        <rect x="33" y="36" width="14" height="10" rx="2" stroke="var(--accent)" stroke-width="1.5"/>
        <path d="M35 36 L35 32 C35 28.7 44.5 28.7 44.5 32 L44.5 36" stroke="var(--accent)" stroke-width="1.5"/>
      </svg>

    {:else}
      <!-- Generic — abstract crosshair -->
      <svg viewBox="0 0 80 72" fill="none" xmlns="http://www.w3.org/2000/svg">
        <circle cx="40" cy="36" r="20" stroke="var(--accent)" stroke-width="1.5"/>
        <circle cx="40" cy="36" r="8"  stroke="var(--accent)" stroke-width="1.5" opacity="0.5"/>
        <path d="M40 6 L40 16 M40 56 L40 66 M10 36 L20 36 M60 36 L70 36"
              stroke="var(--accent)" stroke-width="1.5"/>
      </svg>
    {/if}
  </div>

  <!-- Text block -->
  <div class="text-block">
    <p class="empty-title">{title}</p>
    {#if body}
      <p class="empty-body">{body}</p>
    {/if}
    {#if children}
      <div class="empty-children">{@render children()}</div>
    {/if}
  </div>

  <!-- Actions -->
  {#if actionLabel && action}
    <div class="actions">
      <Button
        variant={isDanger ? 'danger' : 'primary'}
        size="md"
        text={actionLabel}
        onclick={action}
        iconLeading={isDanger ? RotateCcw : undefined}
      />
      {#if secondaryLabel && secondaryAction}
        <Button
          variant="ghost"
          size="md"
          text={secondaryLabel}
          onclick={secondaryAction}
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  /* ── Layout ──────────────────────────────────────────────────────────────── */
  .empty-state {
    display:        flex;
    flex-direction: column;
    align-items:    center;
    justify-content: center;
    text-align:     center;
    gap:            var(--space-5);
    padding:        var(--space-8) var(--space-6);
    width:          100%;
    max-width:      var(--empty-state-max-w, 480px);
    margin:         0 auto;
  }

  .compact {
    padding: var(--space-6) var(--space-5);
    gap:     var(--space-4);
  }

  /* ── Illustration ────────────────────────────────────────────────────────── */
  .illustration {
    --accent: var(--color-accent);
    /* --empty-state-icon-w/h: component-scoped illustration size tokens */
    width:  var(--empty-state-icon-w, 88px);
    height: var(--empty-state-icon-h, 80px);
    flex-shrink: 0;
    opacity: 0.9;
  }
  .illustration svg {
    width:  100%;
    height: 100%;
  }
  .compact .illustration {
    width:  var(--empty-state-icon-w-compact, 64px);
    height: var(--empty-state-icon-h-compact, 58px);
  }

  /* ── Text ────────────────────────────────────────────────────────────────── */
  .text-block {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-2);
  }

  .empty-title {
    margin:       0;
    font-size:    var(--font-size-h2);       /* 20px */
    font-weight:  580;
    color:        var(--color-fg);
    line-height:  1.25;
    letter-spacing: -0.016em;
  }
  .compact .empty-title {
    font-size: var(--font-size-body);        /* 15px */
    font-weight: 520;
  }

  .empty-body {
    margin:      0;
    font-size:   var(--font-size-meta);      /* 13px */
    color:       var(--color-fg-subtle);
    line-height: 1.55;
    max-width:   var(--empty-state-body-max-w, 360px);
  }

  .empty-children {
    margin-top: var(--space-2);
  }

  /* ── Actions ─────────────────────────────────────────────────────────────── */
  .actions {
    display:     flex;
    gap:         var(--space-3);
    align-items: center;
    flex-wrap:   wrap;
    justify-content: center;
  }
</style>
