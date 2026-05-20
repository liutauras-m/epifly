<script lang="ts">
	import { modeStore } from '@conusai/ui/stores';
	import MobileShell from '$lib/mobile/MobileShell.svelte';

	modeStore.setMode('shell');
</script>

<MobileShell />
