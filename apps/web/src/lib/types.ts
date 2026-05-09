export interface WorkspaceNode {
	id: string;
	kind: 'folder' | 'conversation';
	name: string;
	virtual_path: string;
	parent_id: string | null;
	last_modified: string;
}
