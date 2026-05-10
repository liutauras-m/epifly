import { getContext, setContext } from 'svelte';
import {
  createCapabilityRendererRegistry,
  type CapabilityRendererRegistry,
  type CreateRegistryOpts,
} from './CapabilityRendererRegistry.js';

const KEY = Symbol('conusai.capability-registry');

export function provideCapabilityRendererRegistry(opts?: CreateRegistryOpts): CapabilityRendererRegistry {
  const r = createCapabilityRendererRegistry(opts);
  setContext(KEY, r);
  return r;
}

export function useCapabilityRendererRegistry(): CapabilityRendererRegistry {
  const r = getContext<CapabilityRendererRegistry>(KEY);
  if (!r) throw new Error('provideCapabilityRendererRegistry() not called in a parent layout');
  return r;
}
