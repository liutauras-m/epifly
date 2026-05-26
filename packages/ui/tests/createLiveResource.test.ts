import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createLiveResource } from '../src/lib/live/createLiveResource.svelte.js';
import { toasts } from '../src/lib/stores/toast.svelte.js';
import { withRoot } from './helpers/effectRoot.svelte.js';

/**
 * Tests for `createLiveResource` (PR 3.A.4.1 — rollback contract).
 *
 * Each test runs inside a `$effect.root` (via the `withRoot` helper) so the
 * factory's internal `$effect` (visibilitychange + initial refresh) has a
 * tracking scope to run in. Outside a Svelte component, runes need an explicit
 * root — and `$effect.root` itself is rune syntax, so the helper lives in a
 * `.svelte.ts` file rather than this plain `.test.ts`.
 */

type Data = { items: string[] };

describe('createLiveResource — rollback contract (3.A.4.1)', () => {
  beforeEach(() => {
    // Reset toast state between tests so error-assertion checks are clean.
    for (const t of toasts.items) toasts.dismiss(t.id);
    // Suppress the one-shot deprecation warning between runs.
    (globalThis as any)[Symbol.for('createLiveResource.mutateDeprecation')] = { warned: false };
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('reverts data to the pre-mutation snapshot on rollbackOn rejection', async () => {
    const fetcher = vi.fn(async () => ({ items: ['a', 'b'] }) as Data);
    const { result: r, cleanup } = withRoot(() => createLiveResource<Data>('test', fetcher));
    try {
      await r.refresh();
      expect(r.data).toEqual({ items: ['a', 'b'] });

      let rejectFn!: (e: unknown) => void;
      const rollbackOn = new Promise<void>((_, rej) => { rejectFn = rej; });

      r.mutate((d) => ({ items: (d?.items ?? []).filter((x) => x !== 'b') }), { rollbackOn });
      // Optimistic overlay applied immediately.
      expect(r.data).toEqual({ items: ['a'] });

      rejectFn(new Error('boom'));
      // Allow the catch handler to flush.
      await rollbackOn.catch(() => undefined);
      await Promise.resolve();

      expect(r.data).toEqual({ items: ['a', 'b'] });
      expect(r.lastError?.message).toBe('boom');
      const errToast = toasts.items.find((t) => t.kind === 'error');
      expect(errToast?.message).toMatch(/boom/);
    } finally {
      cleanup();
    }
  });

  it('leaves the optimistic value in place on rollbackOn resolution', async () => {
    const fetcher = vi.fn(async () => ({ items: ['x', 'y'] }) as Data);
    const { result: r, cleanup } = withRoot(() => createLiveResource<Data>('test', fetcher));
    try {
      // Settle the factory's auto-init refresh before asserting optimistic state.
      await r.refresh();
      await r.refresh();
      let resolveFn!: () => void;
      const rollbackOn = new Promise<void>((res) => { resolveFn = res; });

      r.mutate((d) => ({ items: [...(d?.items ?? []), 'z'] }), { rollbackOn });
      expect(r.data).toEqual({ items: ['x', 'y', 'z'] });

      resolveFn();
      await rollbackOn;
      await Promise.resolve();

      // Authoritative reconciliation arrives on the next refresh; until then the
      // overlay stays applied.
      expect(r.data).toEqual({ items: ['x', 'y', 'z'] });
      expect(r.lastError).toBeNull();
      expect(toasts.items.find((t) => t.kind === 'error')).toBeUndefined();
    } finally {
      cleanup();
    }
  });

  it('logs exactly one console.warn for the deprecated bare mutate form', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const fetcher = vi.fn(async () => ({ items: ['p'] }) as Data);
    const { result: r, cleanup } = withRoot(() => createLiveResource<Data>('test', fetcher));
    try {
      await r.refresh();
      // Two bare-form calls — only the first must emit a deprecation warning.
      r.mutate((d) => ({ items: [...(d?.items ?? []), 'q'] }));
      r.mutate((d) => ({ items: [...(d?.items ?? []), 'r'] }));

      const deprecationCalls = warn.mock.calls.filter(([msg]) =>
        typeof msg === 'string' && msg.includes('mutate() without { rollbackOn }')
      );
      expect(deprecationCalls).toHaveLength(1);
      expect(r.data).toEqual({ items: ['p', 'q', 'r'] });
    } finally {
      cleanup();
    }
  });

  it('uses errorMessage builder when provided on rejection', async () => {
    const fetcher = vi.fn(async () => ({ items: ['a'] }) as Data);
    const { result: r, cleanup } = withRoot(() => createLiveResource<Data>('test', fetcher));
    try {
      await r.refresh();
      let rejectFn!: (e: unknown) => void;
      const rollbackOn = new Promise<void>((_, rej) => { rejectFn = rej; });

      r.mutate((d) => ({ items: [...(d?.items ?? []), 'b'] }), {
        rollbackOn,
        errorMessage: (e) => `Custom: ${(e as Error).message}`,
      });

      rejectFn(new Error('xyz'));
      await rollbackOn.catch(() => undefined);
      await Promise.resolve();

      const errToast = toasts.items.find((t) => t.kind === 'error');
      expect(errToast?.message).toBe('Custom: xyz');
    } finally {
      cleanup();
    }
  });
});

describe('createLiveResource — scope filtering (3.A.7)', () => {
  it('drops invalidation events whose scope does not match tenantId', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const fetcher = vi.fn(async () => ({ items: ['only'] }) as Data);
    const { result: r, cleanup } = withRoot(() =>
      createLiveResource<Data>('workspace', fetcher, { tenantId: 'tenant-A' }),
    );
    try {
      await r.refresh();
      const initialCalls = fetcher.mock.calls.length;
      r.notifyInvalidationWithScope('workspace', 'tenant-B');
      // No additional fetch should have happened.
      expect(fetcher.mock.calls.length).toBe(initialCalls);
      // A warn was emitted.
      const mismatchWarn = warn.mock.calls.find(([msg]) =>
        typeof msg === 'string' && msg.includes('cross-tenant invalidation'),
      );
      expect(mismatchWarn).toBeDefined();
    } finally {
      cleanup();
    }
  });

  it('forwards invalidation events whose scope matches tenantId', async () => {
    const fetcher = vi.fn(async () => ({ items: ['only'] }) as Data);
    const { result: r, cleanup } = withRoot(() =>
      createLiveResource<Data>('workspace', fetcher, { tenantId: 'tenant-A' }),
    );
    try {
      // Let the auto-init refresh (from the factory's $effect) settle, then
      // the explicit refresh, before counting baseline calls.
      await r.refresh();
      await r.refresh();
      const before = fetcher.mock.calls.length;

      // Allow loading=false to settle before triggering the next fetch.
      r.notifyInvalidationWithScope('workspace', 'tenant-A');
      // notifyInvalidation* fires-and-forgets; flush microtasks so the queued
      // refresh chain settles without depending on timer scheduling.
      await Promise.resolve();
      await Promise.resolve();
      // And let the fetcher's microtask settle.
      await r.refresh();
      expect(fetcher.mock.calls.length).toBeGreaterThan(before);
    } finally {
      cleanup();
    }
  });
});
