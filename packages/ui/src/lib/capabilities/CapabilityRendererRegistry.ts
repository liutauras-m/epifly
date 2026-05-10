import type { Component } from 'svelte';
import type { CapabilityCard } from '@conusai/types';

type Renderer = Component<{ card: CapabilityCard }>;

export interface CapabilityRendererRegistry {
  register(name: string, renderer: Renderer): void;
  unregister(name: string): void;
  get(card: CapabilityCard): Renderer | null;
  readonly names: readonly string[];
}

export interface CreateRegistryOpts {
  fallbackRenderer?: Renderer;
}

export function createCapabilityRendererRegistry(opts: CreateRegistryOpts = {}): CapabilityRendererRegistry {
  const renderers = new Map<string, Renderer>();
  return {
    register(name, renderer) { renderers.set(name, renderer); },
    unregister(name)         { renderers.delete(name); },
    get(card)                { return renderers.get(card.name) ?? opts.fallbackRenderer ?? null; },
    get names()              { return Array.from(renderers.keys()); },
  };
}
