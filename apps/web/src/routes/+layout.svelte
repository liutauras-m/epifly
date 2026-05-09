<script lang="ts">
	import '../app.css';
	import LiveAnnouncer from '$lib/ui/LiveAnnouncer.svelte';
	import type { LayoutData } from './$types';
	let { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();
</script>

{@render children()}
<LiveAnnouncer />
