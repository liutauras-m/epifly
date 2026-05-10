import { describe, it, expect, vi } from 'vitest';
import { createCapabilityRendererRegistry } from '../src/lib/capabilities/CapabilityRendererRegistry.js';
import type { CapabilityCard } from '@conusai/types';

function card(name: string): CapabilityCard {
  return { capability_id: name, name, description: '', kind: 'builtin', tenant_scope: [], tags: [] };
}

describe('createCapabilityRendererRegistry', () => {
  it('returns null for unknown capability', () => {
    const reg = createCapabilityRendererRegistry();
    expect(reg.get(card('unknown'))).toBeNull();
  });

  it('registers and retrieves a renderer', () => {
    const reg = createCapabilityRendererRegistry();
    const renderer = vi.fn() as any;
    reg.register('search', renderer);
    expect(reg.get(card('search'))).toBe(renderer);
  });

  it('unregisters a renderer', () => {
    const reg = createCapabilityRendererRegistry();
    const renderer = vi.fn() as any;
    reg.register('search', renderer);
    reg.unregister('search');
    expect(reg.get(card('search'))).toBeNull();
  });

  it('returns fallbackRenderer when no match', () => {
    const fallback = vi.fn() as any;
    const reg = createCapabilityRendererRegistry({ fallbackRenderer: fallback });
    expect(reg.get(card('no-match'))).toBe(fallback);
  });

  it('registered renderer takes priority over fallback', () => {
    const fallback = vi.fn() as any;
    const specific = vi.fn() as any;
    const reg = createCapabilityRendererRegistry({ fallbackRenderer: fallback });
    reg.register('chart', specific);
    expect(reg.get(card('chart'))).toBe(specific);
  });

  it('exposes names of all registered capabilities', () => {
    const reg = createCapabilityRendererRegistry();
    reg.register('a', vi.fn() as any);
    reg.register('b', vi.fn() as any);
    expect(reg.names).toEqual(expect.arrayContaining(['a', 'b']));
    expect(reg.names).toHaveLength(2);
  });

  it('names updates after unregister', () => {
    const reg = createCapabilityRendererRegistry();
    reg.register('x', vi.fn() as any);
    reg.register('y', vi.fn() as any);
    reg.unregister('x');
    expect(reg.names).toEqual(['y']);
  });

  it('canary: dynamically registered renderer is immediately available (mirrors Rust registry symmetry)', () => {
    const reg = createCapabilityRendererRegistry();
    expect(reg.get(card('trace-replay'))).toBeNull();
    const traceReplay = vi.fn() as any;
    reg.register('trace-replay', traceReplay);
    expect(reg.get(card('trace-replay'))).toBe(traceReplay);
  });
});
