import { flushSync } from 'svelte';

/**
 * Run `fn()` inside a Svelte `$effect.root` so any nested `$effect` calls have
 * a tracking scope. Returns the factory result + a `cleanup()` to dispose the
 * root (must be called by the test or runes leak across cases).
 *
 * Lives in a `.svelte.ts` file because `$effect.root` is rune syntax and only
 * the Svelte compiler can lower it. Plain `.ts` test files cannot use runes.
 */
export function withRoot<T>(fn: () => T): { result: T; cleanup: () => void } {
  let result!: T;
  const cleanup = $effect.root(() => {
    result = fn();
  });
  flushSync();
  return { result, cleanup };
}
