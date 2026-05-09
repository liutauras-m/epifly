/**
 * Runes-based workspace context store.
 * Set once at the root (WorkspaceTree) via setContext; consumed anywhere
 * in the workspace subtree without prop-drilling.
 */
import { getContext, setContext } from 'svelte';
import type { WorkspaceNode } from '$lib/types';

const CTX_KEY = Symbol('workspace');

export interface WorkspaceContext {
	readonly tree: WorkspaceNode[];
	readonly childMap: Map<string, WorkspaceNode[]>;
	readonly expanded: Set<string>;
	readonly selectedId: string | null;
	readonly searchQuery: string;
	readonly searchResults: WorkspaceNode[];
	setTree(nodes: WorkspaceNode[]): void;
	setChildren(parentId: string, nodes: WorkspaceNode[]): void;
	toggleExpanded(id: string): void;
	setSelected(id: string | null): void;
	setSearch(q: string, results: WorkspaceNode[]): void;
	patchNode(updated: WorkspaceNode): void;
}

export function createWorkspaceContext(): WorkspaceContext {
	let tree = $state<WorkspaceNode[]>([]);
	let childMap = $state<Map<string, WorkspaceNode[]>>(new Map());
	let expanded = $state<Set<string>>(new Set());
	let selectedId = $state<string | null>(null);
	let searchQuery = $state('');
	let searchResults = $state<WorkspaceNode[]>([]);

	const ctx: WorkspaceContext = {
		get tree() { return tree; },
		get childMap() { return childMap; },
		get expanded() { return expanded; },
		get selectedId() { return selectedId; },
		get searchQuery() { return searchQuery; },
		get searchResults() { return searchResults; },
		setTree(nodes) { tree = nodes; },
		setChildren(parentId, nodes) {
			const m = new Map(childMap);
			m.set(parentId, nodes);
			childMap = m;
		},
		toggleExpanded(id) {
			const s = new Set(expanded);
			if (s.has(id)) s.delete(id); else s.add(id);
			expanded = s;
		},
		setSelected(id) { selectedId = id; },
		setSearch(q, results) { searchQuery = q; searchResults = results; },
		patchNode(updated) {
			function patch(nodes: WorkspaceNode[]): boolean {
				for (let i = 0; i < nodes.length; i++) {
					if (nodes[i].id === updated.id) { nodes[i] = updated; return true; }
				}
				return false;
			}
			if (!patch(tree)) {
				for (const children of childMap.values()) patch(children);
			}
			tree = [...tree];
		},
	};
	return ctx;
}

export function provideWorkspaceContext(ctx: WorkspaceContext) {
	setContext(CTX_KEY, ctx);
}

export function useWorkspaceContext(): WorkspaceContext {
	return getContext<WorkspaceContext>(CTX_KEY);
}
