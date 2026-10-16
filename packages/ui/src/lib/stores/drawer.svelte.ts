/**
 * Drawer / sidebar visibility store.
 *
 * On desktop the drawer is always "open" and rendered as a persistent sidebar.
 * On mobile the drawer is an overlay that slides in from the left; `open`
 * controls that overlay — desktop ignores it via CSS.
 */
export function createDrawerStore() {
	let open = $state(false);

	return {
		get open() { return open; },
		set open(v: boolean) { open = v; },
		toggle() { open = !open; },
		close() { open = false; },
	};
}

export type DrawerStore = ReturnType<typeof createDrawerStore>;
