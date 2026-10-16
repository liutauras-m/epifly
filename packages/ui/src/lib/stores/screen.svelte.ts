/**
 * Screen navigation store — the active top-level screen
 * (chat / capabilities / artifacts) and a back stack.
 *
 * One instance per app tree (see appStores.svelte.ts): never module-level, so
 * SSR requests cannot see each other's navigation state.
 */

export type Screen = 'chat' | 'capabilities' | 'artifacts';

export function createScreenStore() {
	let active = $state<Screen>('chat');
	let stack = $state<Screen[]>([]);

	return {
		get active() { return active; },
		get canGoBack() { return stack.length > 0; },
		/** Replace the current screen (clears back stack). */
		setActive(s: Screen) { stack = []; active = s; },
		/** Push current to stack and switch to new screen. */
		push(s: Screen) { stack = [...stack, active]; active = s; },
		/** Pop the stack and return to the previous screen. */
		pop() {
			if (stack.length === 0) return;
			active = stack[stack.length - 1];
			stack = stack.slice(0, -1);
		},
	};
}

export type ScreenStore = ReturnType<typeof createScreenStore>;
