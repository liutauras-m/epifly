<script lang="ts">
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { modeStore } from '@conusai/ui/stores';
	import TraceReplayCapability from '$lib/TraceReplayCapability.svelte';
	import MobileShell from '$lib/mobile/MobileShell.svelte';

	modeStore.setMode('shell');

	const registry = provideCapabilityRendererRegistry();
	registry.register('trace.replay', TraceReplayCapability as any);
</script>

<MobileShell />
