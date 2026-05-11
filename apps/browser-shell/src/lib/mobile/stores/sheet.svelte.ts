export interface SheetEntry {
	key: string;
	props?: Record<string, unknown>;
}

let stack = $state<SheetEntry[]>([]);

export const sheetStore = {
	get top(): SheetEntry | null { return stack[stack.length - 1] ?? null; },
	get stack() { return stack; },
	push(entry: SheetEntry) { stack = [...stack, entry]; },
	pop() { stack = stack.slice(0, -1); },
	close() { stack = []; },
};
