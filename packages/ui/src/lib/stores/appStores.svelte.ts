/**
 * Per-app-tree UI state: navigation, drawer, breadcrumbs, recents, toasts.
 *
 * The stores live in Svelte context, created by `provideAppStores()` in the
 * root layout. Nothing is module-level, so on the server every request
 * renders with fresh state and no user's navigation, toasts or breadcrumbs
 * can leak into another user's SSR response.
 */
import { getContext, setContext } from 'svelte';
import { createBreadcrumbsStore, type BreadcrumbsStore } from './breadcrumbs.svelte.js';
import { createDrawerStore, type DrawerStore } from './drawer.svelte.js';
import { createRecentsStore, type RecentsStore } from './recents.svelte.js';
import { createScreenStore, type ScreenStore } from './screen.svelte.js';
import { createToastStore, type ToastStore } from './toast.svelte.js';

export interface AppStores {
	screen: ScreenStore;
	drawer: DrawerStore;
	breadcrumbs: BreadcrumbsStore;
	recents: RecentsStore;
	toasts: ToastStore;
}

const KEY = Symbol('conusai.app-stores');

export function createAppStores(): AppStores {
	return {
		screen: createScreenStore(),
		drawer: createDrawerStore(),
		breadcrumbs: createBreadcrumbsStore(),
		recents: createRecentsStore(),
		toasts: createToastStore(),
	};
}

/** Call once in the root layout's component init. */
export function provideAppStores(stores: AppStores = createAppStores()): AppStores {
	setContext(KEY, stores);
	return stores;
}

/** The app tree's stores. Must run during component init under the root layout. */
export function useAppStores(): AppStores {
	const stores = getContext<AppStores | undefined>(KEY);
	if (!stores) throw new Error('useAppStores(): call provideAppStores() in the root layout first');
	return stores;
}
