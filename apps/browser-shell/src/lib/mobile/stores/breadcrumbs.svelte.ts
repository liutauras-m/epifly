import type { WorkspaceNode } from '@conusai/types';

let node = $state<WorkspaceNode | null>(null);

export const breadcrumbsStore = {
	get node() { return node; },
	set(n: WorkspaceNode | null) { node = n; },
	clear() { node = null; },
};
