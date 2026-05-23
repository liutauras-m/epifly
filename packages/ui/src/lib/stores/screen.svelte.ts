/**
 * Screen navigation store — manages the active top-level screen
 * (chat / capabilities / artifacts) and a navigation stack for back-button support.
 *
 * Shared between web and shell so both apps use the same screen model.
 */

export type Screen = 'chat' | 'capabilities' | 'artifacts';

let active = $state<Screen>('chat');
let stack = $state<Screen[]>([]);

export const screenStore = {
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
