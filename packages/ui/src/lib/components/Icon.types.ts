import type { Component, ComponentType } from 'svelte';

export type IconSize = 'sm' | 'md' | 'lg';

/** Accepts both Svelte 5 function-components and legacy class-components. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type IconComponent = Component<any> | ComponentType<any>;
