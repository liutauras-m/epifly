<script lang="ts">
	import type { WorkspaceNode } from '@conusai/types';

	let { node }: { node: WorkspaceNode | null } = $props();
</script>

{#if node}
	<span class="breadcrumb">{node.name}</span>
{:else}
	<span class="breadcrumb muted">Chat</span>
{/if}

<style>
	.breadcrumb {
		font-family: var(--font-family-sans);
		font-size: 16px;
		font-weight: 600;
		color: var(--ink);
		letter-spacing: -0.3px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.muted { color: var(--ink-2); }
</style>
