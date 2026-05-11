/**
 * Svelte action that assigns `--stagger-i` (index) and `--stagger-delay` (ms)
 * CSS custom properties to each child of the host element, so children can
 * animate in choreographed sequence via `animation-delay: var(--stagger-delay)`.
 *
 * Default delay is 60ms per item — matches the Epifly motion guideline
 * (≤ 60ms per item, ≤ 300ms total stagger).
 */
export function stagger(node: HTMLElement, opts: { delay?: number } = {}) {
  const delay = opts.delay ?? 60;
  function apply() {
    Array.from(node.children).forEach((child, i) => {
      (child as HTMLElement).style.setProperty("--stagger-i", String(i));
      (child as HTMLElement).style.setProperty("--stagger-delay", `${i * delay}ms`);
      (child as HTMLElement).style.animationDelay = `${i * delay}ms`;
    });
  }
  apply();
  const mo = new MutationObserver(apply);
  mo.observe(node, { childList: true });
  return {
    destroy() {
      mo.disconnect();
    },
  };
}
