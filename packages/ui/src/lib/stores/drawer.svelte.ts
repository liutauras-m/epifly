/**
 * Drawer / sidebar visibility store.
 *
 * On desktop the drawer is always "open" and rendered as a persistent sidebar.
 * On mobile the drawer is overlay that slides in from the left.
 *
 * `open` controls the mobile overlay state — desktop ignores it via CSS.
 */

let open = $state(false);

export const drawerStore = {
	get open() { return open; },
	toggle() { open = !open; },
	close() { open = false; },
	open_() { open = true; },
};
