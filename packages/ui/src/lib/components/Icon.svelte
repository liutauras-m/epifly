<svelte:options runes={true} />
<script lang="ts">
  /**
   * Icon — iconography primitive (Phase 2.4).
   *
   * Wraps any lucide-svelte icon with Foundry's enforced defaults:
   *   • stroke-width 1.5 (Foundry standard — never raw stroke attrs in components)
   *   • size from token: --icon-sm 16, --icon-md 20, --icon-lg 24
   *   • aria-hidden="true" by default (decorative); pass label for standalone icons
   *
   * Usage:
   *   import { Search } from 'lucide-svelte';
   *   <Icon icon={Search} />                    ← md (20px), decorative
   *   <Icon icon={Search} size="lg" />          ← lg (24px)
   *   <Icon icon={Search} size={32} />          ← explicit px
   *   <Icon icon={Search} label="Search" />     ← standalone — adds role="img"
   */
  import type { Component } from 'svelte';

  export type IconSize = 'sm' | 'md' | 'lg';

  const SIZE_PX: Record<IconSize, number> = { sm: 16, md: 20, lg: 24 };

  let {
    icon:       IconComponent,
    size   = 'md' as IconSize | number,
    label,
    class: cls = '',
    strokeWidth = 1.5,
    color,
  }: {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon:         Component<any>;
    size?:        IconSize | number;
    /** Accessible label — when provided the icon gets role="img" instead of aria-hidden. */
    label?:       string;
    class?:       string;
    strokeWidth?: number;
    color?:       string;
  } = $props();

  const px  = $derived(typeof size === 'number' ? size : SIZE_PX[size]);
  const a11y = $derived(
    label
      ? { role: 'img' as const, 'aria-label': label }
      : { 'aria-hidden': 'true' as const },
  );
</script>

<span class="icon icon-{typeof size === 'string' ? size : 'custom'}{cls ? ` ${cls}` : ''}" {...a11y}>
  <IconComponent size={px} strokeWidth={strokeWidth} color={color ?? 'currentColor'} />
</span>

<style>
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    line-height: 0;
  }

  /* Size classes map to CSS custom property tokens */
  .icon-sm { width: var(--icon-sm, 16px); height: var(--icon-sm, 16px); }
  .icon-md { width: var(--icon-md, 20px); height: var(--icon-md, 20px); }
  .icon-lg { width: var(--icon-lg, 24px); height: var(--icon-lg, 24px); }
</style>
