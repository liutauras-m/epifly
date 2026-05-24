import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

/** Type helper: adds `ref` bindable to any HTML element type.
 *  The constraint is intentionally loose — shadcn components use specific
 *  element attribute types (HTMLButtonAttributes, HTMLDivAttributes, etc.)
 *  which are subsets, not extensions, of HTMLAttributes<HTMLElement>. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithElementRef<T extends Record<string, any>, E extends HTMLElement = HTMLElement> = T & {
	ref?: E | null;
};

/** Removes `children` and `child` snippet props from a component's props type.
 *  Used by shadcn Tooltip.Content, Sheet.Content, etc. when wrapping headless Bits UI. */
export type WithoutChildren<T> = T extends { children?: unknown } ? Omit<T, 'children'> : T;
export type WithoutChild<T> = T extends { child?: unknown } ? Omit<T, 'child'> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
