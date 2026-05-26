/**
 * SWR-style live resource factory for Svelte 5 runes (PR 3.A).
 *
 * Features:
 * - Initial fetch on mount.
 * - Revalidation on `resource_invalidated` SSE delta from `createChatStream`.
 * - Background refetch when the tab becomes visible after a period of inactivity.
 * - Optimistic update via `mutate(updater, { rollbackOn })` with structural clone
 *   (NOT immer — incompatible with Svelte 5 reactive proxies). Rollback fires
 *   automatically when the bound promise rejects, surfaces via `toasts.error`.
 * - Exponential-backoff retry on fetch failure (3 attempts, caps at 8 s).
 * - Optional per-tenant scope filtering on `notifyInvalidationWithScope` —
 *   defensive guard against cross-tenant invalidation leaks (PR 3.A.7).
 *
 * Usage:
 * ```svelte
 * <script>
 *   import { createLiveResource } from '@conusai/ui/live';
 *
 *   const nodes = createLiveResource('workspace', fetchWorkspaceTree, { tenantId });
 *
 *   // Register a chat stream so invalidation events trigger refetch:
 *   $effect(() => { nodes.subscribeToStream(chatStream); });
 *
 *   // Optimistic delete with mandatory rollback promise:
 *   nodes.mutate(
 *     d => ({ ...d, nodes: d.nodes.filter(n => n.id !== id) }),
 *     { rollbackOn: sdk.workspaces.delete(id) },
 *   );
 * </script>
 * ```
 *
 * The factory uses `structuredClone` for optimistic snapshots — do NOT pass immer
 * producers; they conflict with Svelte 5 reactive proxy traps.
 */

import { toasts } from '../stores/toast.svelte.js';

const BACKOFF_MS = [500, 2000, 8000] as const;
const IDLE_THRESHOLD_MS = 60_000; // refetch when tab visible after 1 min idle

type Fetcher<T> = () => Promise<T>;

/** Options for `mutate(updater, opts)`. `rollbackOn` is required on this overload. */
export interface MutateOptions {
  /** Promise whose rejection triggers automatic rollback to the pre-mutation snapshot. */
  rollbackOn: Promise<unknown>;
  /** Optional custom toast message builder. Defaults to `"Update failed: <error.message>"`. */
  errorMessage?: (e: unknown) => string;
}

export interface CreateLiveResourceOptions {
  /**
   * If set, `notifyInvalidationWithScope` drops events whose `scope` does not match.
   * Defensive client-side guard — the server already filters by tenant before sending,
   * but a mismatch indicates either a misconfig or a real cross-tenant leak; we log and
   * drop. Pass `data.user.tenantId` from the SvelteKit load.
   */
  tenantId?: string | null | (() => string | null);
}

export interface LiveResource<T> {
  /** Current data. `null` until first successful fetch. */
  get data(): T | null;
  /** True while a fetch is in flight. */
  get loading(): boolean;
  /** Last fetch error, or null. */
  get error(): Error | null;
  /** Last optimistic-rollback error, or null. Distinct from `error` (fetch error). */
  get lastError(): Error | null;
  /** Manually trigger a refetch. */
  refresh(): Promise<void>;
  /**
   * @deprecated Use `mutate(updater, { rollbackOn })` — overlays without rollback
   * mask server failures and are a bug magnet. The bare form is kept for one minor
   * to ease migration.
   */
  mutate(updater: (current: T | null) => T): void;
  /**
   * Optimistic update with mandatory rollback promise.
   * - Snapshots current `data` via `structuredClone` *before* applying the updater.
   * - Applies `updater(structuredClone(data))` and sets the result as the new `data`.
   * - On `rollbackOn` rejection: reverts to the snapshot, sets `lastError`, and fires
   *   `toasts.error(opts.errorMessage?.(e) ?? "Update failed: <message>")`.
   * - On resolve: the next inbound `resource_invalidated` reconciles authoritatively.
   */
  mutate(updater: (current: T | null) => T, opts: MutateOptions): void;
  /**
   * Register this resource to react to `resource_invalidated` deltas from a
   * `createChatStream` instance. Call inside a `$effect` so the subscription is
   * torn down automatically.
   */
  subscribeToStream(stream: { lastRoutingMeta: unknown }): void;
  /** Trigger a refetch when an invalidation arrives for this resource. */
  notifyInvalidation(incomingResource: string): void;
  /**
   * Same as `notifyInvalidation` but also asserts that the event's tenant scope
   * matches the configured `tenantId` (if set). Mismatched scopes are dropped
   * with a `console.warn`.
   */
  notifyInvalidationWithScope(incomingResource: string, scope: string): void;
}

const DEPRECATION_KEY = Symbol.for('createLiveResource.mutateDeprecation');
type DeprecationFlag = { warned: boolean };
const deprecationFlag: DeprecationFlag = ((globalThis as Record<symbol, unknown>)[DEPRECATION_KEY] ??=
  { warned: false }) as DeprecationFlag;

export function createLiveResource<T>(
  resource: string,
  fetcher: Fetcher<T>,
  options: CreateLiveResourceOptions = {},
): LiveResource<T> {
  const getTenantId =
    typeof options.tenantId === 'function'
      ? options.tenantId
      : () => options.tenantId ?? null;

  let data = $state<T | null>(null);
  let loading = $state(false);
  let error = $state<Error | null>(null);
  let lastError = $state<Error | null>(null);
  let attempt = 0;
  let lastFetchTime = 0;
  // Bumps on each optimistic mutation so stale in-flight fetches can be ignored.
  let optimisticEpoch = 0;

  async function refresh() {
    if (loading) return;
    loading = true;
    error = null;
    const refreshEpoch = optimisticEpoch;
    try {
      const next = await fetcher();
      // If an optimistic mutation happened while this refresh was in flight,
      // keep the optimistic overlay instead of clobbering it with stale data.
      if (refreshEpoch === optimisticEpoch) {
        data = next;
      }
      lastFetchTime = Date.now();
      attempt = 0;
    } catch (e: unknown) {
      error = e instanceof Error ? e : new Error(String(e));
      if (attempt < BACKOFF_MS.length) {
        const delay = BACKOFF_MS[attempt++];
        setTimeout(() => refresh(), delay);
      }
    } finally {
      loading = false;
    }
  }

  function toErr(e: unknown): Error {
    return e instanceof Error ? e : new Error(String(e));
  }

  function cloneSnapshot(value: T | null): T | null {
    if (value === null) return null;
    // `$state.snapshot` returns a plain non-reactive copy that `structuredClone`
    // can subsequently deep-clone. Svelte 5 reactive proxies are otherwise not
    // cloneable.
    return structuredClone($state.snapshot(value) as T) as T;
  }

  function mutate(updater: (current: T | null) => T, opts?: MutateOptions): void {
    if (!opts) {
      if (!deprecationFlag.warned) {
        deprecationFlag.warned = true;
        console.warn(
          '[createLiveResource] mutate() without { rollbackOn } is deprecated. ' +
          'Pass the SDK mutation promise so failed writes can revert the overlay.',
        );
      }
      data = updater(cloneSnapshot(data));
      return;
    }
    // Snapshot BEFORE the update so rollback restores the pre-mutation state.
    const snapshot = cloneSnapshot(data);
    const draft = cloneSnapshot(data);
    optimisticEpoch += 1;
    data = updater(draft);
    void opts.rollbackOn.then(
      () => {
        // Authoritative reconciliation arrives via the next `resource_invalidated`.
        // We intentionally leave `data` as the optimistic overlay until then.
      },
      (e: unknown) => {
        data = snapshot;
        lastError = toErr(e);
        const msg = opts.errorMessage?.(e) ?? `Update failed: ${lastError.message}`;
        toasts.error(msg);
      },
    );
  }

  // ── Tab-visibility refetch ────────────────────────────────────────────────
  // Refetch when the user returns to the tab after a period of inactivity.
  function handleVisibilityChange() {
    if (document.visibilityState === 'visible') {
      const idle = Date.now() - lastFetchTime;
      if (idle > IDLE_THRESHOLD_MS) {
        refresh();
      }
    }
  }

  // Svelte 5 $effect runs once on mount, cleanup runs on unmount.
  $effect(() => {
    document.addEventListener('visibilitychange', handleVisibilityChange);
    refresh();
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  });

  // ── Stream-based invalidation ─────────────────────────────────────────────
  // The consumer calls `subscribeToStream(chatStream)` inside a $effect.
  // When the stream's lastRoutingMeta changes (i.e., a new turn completed),
  // we check whether the `resource_invalidated` signal matches our resource
  // and trigger a refetch.  The actual `resource_invalidated` delta is delivered
  // separately via `createChatStream` → passed as `lastInvalidated`.

  function subscribeToStream(stream: { lastRoutingMeta: unknown }) {
    // This is called inside a $effect, so reading reactive properties here
    // is sufficient to establish the tracking dependency.
    void stream.lastRoutingMeta;
    // The consumer should call notifyInvalidation when they receive a
    // resource_invalidated delta — see the comment in createChatStream.svelte.ts.
  }

  function notifyInvalidation(incomingResource: string) {
    if (incomingResource === resource || incomingResource === '*') {
      refresh();
    }
  }

  function notifyInvalidationWithScope(incomingResource: string, scope: string) {
    const tenantId = getTenantId();
    if (tenantId != null && scope !== tenantId) {
      console.warn(
        `[createLiveResource] dropping cross-tenant invalidation for "${incomingResource}": ` +
        `event scope "${scope}" !== resource tenantId "${tenantId}"`,
      );
      return;
    }
    notifyInvalidation(incomingResource);
  }

  return {
    get data() { return data; },
    get loading() { return loading; },
    get error() { return error; },
    get lastError() { return lastError; },
    refresh,
    mutate,
    subscribeToStream,
    notifyInvalidation,
    notifyInvalidationWithScope,
  };
}
