/** Options accepted by the autoGrow Svelte action. */
export type AutoGrowOptions =
  | number
  | { maxRows?: number; lineHeight?: number; maxHeight?: number };

/**
 * autoGrow — Svelte action that auto-expands a textarea up to a max height.
 *
 * Accepts either:
 *   use:autoGrow            — default maxHeight 240 px
 *   use:autoGrow={320}      — maxHeight in px
 *   use:autoGrow={{ maxRows: 8, lineHeight: 24 }}  — row-based limit
 */
export function autoGrow(node: HTMLTextAreaElement, opts: AutoGrowOptions = 240) {
  function resolveMaxHeight(o: AutoGrowOptions): number {
    if (typeof o === 'number') return o;
    if (o.maxHeight != null) return o.maxHeight;
    const lh = o.lineHeight ?? 24;
    const rows = o.maxRows ?? 10;
    return lh * rows;
  }

  function resize() {
    const max = resolveMaxHeight(opts);
    node.style.height = 'auto';
    node.style.height = Math.min(node.scrollHeight, max) + 'px';
  }

  node.addEventListener('input', resize);
  const observer = new MutationObserver(resize);
  observer.observe(node, { attributes: true, attributeFilter: ['value'] });
  resize();

  return {
    update(newOpts: AutoGrowOptions) {
      opts = newOpts;
      resize();
    },
    destroy() {
      node.removeEventListener('input', resize);
      observer.disconnect();
    },
  };
}
