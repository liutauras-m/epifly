<svelte:options runes={true} />
<script lang="ts">
  /**
   * Type — typography primitive (Phase 2.2).
   *
   * The ONLY place `font-variation-settings` lives in this repo.
   * Body copy uses semantic elements (`<p>`, `<li>`, …) with the
   * `t-body` / `t-body-strong` utility classes from foundry.css instead.
   *
   * Usage:
   *   <Type variant="h1">Section title</Type>
   *   <Type variant="display" tag="h1">Hero heading</Type>  ← override element
   *   <Type variant="label" class="section-eyebrow">RECENT</Type>
   */
  import type { Snippet } from 'svelte';

  export type TypeVariant = 'display' | 'h1' | 'h2' | 'h3' | 'label' | 'meta' | 'mono';

  // Default semantic element for each variant
  const TAG: Record<TypeVariant, string> = {
    display: 'p',
    h1:      'h1',
    h2:      'h2',
    h3:      'h3',
    label:   'span',
    meta:    'span',
    mono:    'code',
  };

  let {
    variant,
    tag,
    class: cls = '',
    /** Convenience plain-text prop — useful in fixture files and programmatic contexts.
     *  Real usage should prefer passing children as a snippet. */
    text,
    children,
    ...rest
  }: {
    variant: TypeVariant;
    /** Override the rendered HTML element when the semantic context requires it. */
    tag?: string;
    class?: string;
    text?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const element = $derived(tag ?? TAG[variant]);
</script>

<svelte:element this={element} class="type type-{variant}{cls ? ` ${cls}` : ''}" {...rest}>
  {#if children}{@render children()}{:else}{text ?? ''}{/if}
</svelte:element>

<style>
  /*
   * ALL font-variation-settings live here — never duplicated in consuming CSS.
   * Geist is a variable font with `wght` (100–900) and `opsz` (6–144) axes.
   * Body/paragraph text uses foundry.css utility classes (t-body, t-body-strong)
   * and semantic elements, NOT this component.
   */

  .type {
    margin: 0;
    font-family: var(--font-family-sans);
    color: inherit;
  }

  /* display — hero headings: large, tight, high optical size */
  .type-display {
    font-size: var(--font-size-display);   /* clamp(40px, 5.4vw, 56px) */
    line-height: 1.1;
    letter-spacing: -0.03em;
    font-variation-settings: "wght" 640, "opsz" 36;
  }

  /* h1 — section headers */
  .type-h1 {
    font-size: var(--font-size-h1);        /* 28px */
    line-height: 1.2;
    letter-spacing: -0.025em;
    font-variation-settings: "wght" 620, "opsz" 28;
  }

  /* h2 — subsection headers */
  .type-h2 {
    font-size: var(--font-size-h2);        /* 20px */
    line-height: 1.3;
    letter-spacing: -0.018em;
    font-variation-settings: "wght" 580, "opsz" 20;
  }

  /* h3 — card / panel titles */
  .type-h3 {
    font-size: var(--font-size-h2);        /* same size, lighter weight */
    line-height: 1.35;
    letter-spacing: -0.012em;
    font-variation-settings: "wght" 520, "opsz" 18;
  }

  /* label — eyebrows, captions, ALL-CAPS metadata */
  .type-label {
    font-size: var(--font-size-label);     /* 11px */
    line-height: 1.4;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-variation-settings: "wght" 580;
    color: var(--ink-3);
  }

  /* meta — secondary text, timestamps, badges */
  .type-meta {
    font-size: var(--font-size-meta);      /* 13px */
    line-height: 1.5;
    font-variation-settings: "wght" 400;
    color: var(--ink-2);
  }

  /* mono — code, IDs, monospaced values */
  .type-mono {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-mono);      /* 13px */
    line-height: 1.5;
    letter-spacing: 0;
  }
</style>
