<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import CapabilityBrowser from '../CapabilityBrowser.svelte';
	import CapabilityDetailSheet from './CapabilityDetailSheet.svelte';
	import type { CapEntry } from '../CapabilityBrowser.svelte';

	let {
		sdk,
		onInvoke,
	}: {
		sdk: ConusSdk;
		/** Called when the user picks a capability to invoke. Receives the full capability. */
		onInvoke: (cap: CapEntry) => void;
	} = $props();

	let selectedCap = $state<CapEntry | null>(null);
	let sheetOpen = $state(false);

	function openDetail(cap: CapEntry) {
		selectedCap = cap;
		sheetOpen = true;
	}
</script>

<div class="caps-screen">
	<CapabilityBrowser
		{sdk}
		onSelect={openDetail}
		showChevron={true}
	/>
</div>

<CapabilityDetailSheet
	open={sheetOpen}
	capability={selectedCap}
	onClose={() => sheetOpen = false}
	onInvoke={(cap) => { onInvoke(cap); sheetOpen = false; }}
/>

<style>
	.caps-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--paper);
	}
</style>
