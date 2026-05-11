export type Screen = 'chat' | 'capabilities' | 'artifacts';

let active = $state<Screen>('chat');
let stack = $state<Screen[]>([]);

export const screenStore = {
	get active() { return active; },
	get canGoBack() { return stack.length > 0; },
	setActive(s: Screen) { stack = []; active = s; },
	push(s: Screen) { stack = [...stack, active]; active = s; },
	pop() {
		if (stack.length === 0) return;
		active = stack[stack.length - 1];
		stack = stack.slice(0, -1);
	},
};
