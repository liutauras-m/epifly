/**
 * Returns true when the user has requested reduced motion at the OS level.
 *
 * SSR-safe: returns `false` when `window` is unavailable so the server-rendered
 * markup matches the client default (motion enabled). Components that need to
 * differ between server and client should still gate behind onMount / $effect.
 */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}
