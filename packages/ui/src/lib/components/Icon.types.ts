import type { Component, ComponentType } from 'svelte';

export type IconSize = 'sm' | 'md' | 'lg';

/**
 * Accepts both Svelte 5 function-components (Component<any>) and
 * Svelte 4 class-components (SvelteComponentTyped subclasses) as used
 * by lucide-svelte ≤ 0.477.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type IconComponent = Component<any> | ComponentType<any>;
