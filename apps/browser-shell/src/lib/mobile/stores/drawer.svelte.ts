let open = $state(false);

export const drawerStore = {
	get open() { return open; },
	toggle() { open = !open; },
	close() { open = false; },
	open_() { open = true; },
};
