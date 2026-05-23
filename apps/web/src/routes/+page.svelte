<svelte:options runes={true} />
<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import type { PageData } from './$types';
	import type { WorkspaceNode } from '@conusai/types';
	import { sdk } from '$lib/sdk';
	import {
		toasts,
		recentsStore,
		screenStore,
		drawerStore,
		breadcrumbsStore,
	} from '@conusai/ui/stores';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import {
		WorkspaceExplorer,
		ChatScreen,
		CapabilitiesScreen,
		ArtifactsScreen,
		DrawerRecentChats,
		createChatStream,
		buildInvocationPrompt,
		initialRoute,
		applyInitialRoute,
		type CapEntry,
	} from '@conusai/ui/features';
	import {
		AppShell,
		AppHeader,
		Drawer,
		Sidebar,
		SidebarSection,
		SidebarItem,
		ThemeSwitcher,
		Icon,
		Button,
	} from '@conusai/ui';
	import { LayoutGrid, FileText, Menu, ArrowLeft, Plus } from 'lucide-svelte';
	import { registerKeyboardShortcuts } from '@conusai/ui/utils/keyboard.js';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	let { data }: { data: PageData } = $props();

	const registry = provideCapabilityRendererRegistry();
	const chatStream = createChatStream(sdk, { tenantId: data.user?.tenantId ?? null });

	// ── Workspace state ──────────────────────────────────────────────
	let workspaceNodes = $state<WorkspaceNode[]>(data.workspaceTree ?? []);
	let selectedNodeId = $state<string | undefined>();
	let selectedNode = $state<WorkspaceNode | null>(null);
	let composerRef = $state<{ focus(): void } | undefined>();

	// ── Derived screen title (mirrors shell) ─────────────────────────
	const screenTitle = $derived(
		screenStore.active === 'capabilities' ? 'Capabilities' :
		screenStore.active === 'artifacts'    ? 'Artifacts' :
		breadcrumbsStore.node?.name ?? 'Workshop'
	);

	// ── Navigation ───────────────────────────────────────────────────
	function onSelectNode(node: WorkspaceNode) {
		selectedNode = node;
		selectedNodeId = node.id;
		breadcrumbsStore.set(node);
		recentsStore.add(node.id);
		screenStore.setActive('chat');
		if (node.kind === 'conversation') {
			if ((node.metadata as any)?.thread_id) {
				chatStream.loadThread((node.metadata as any).thread_id as string);
			} else {
				chatStream.newSession();
			}
		}
		// Auto-close drawer on mobile after selection
		drawerStore.close();
		goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true });
	}

	function handleNewChat() {
		chatStream.newSession();
		selectedNode = null;
		selectedNodeId = undefined;
		breadcrumbsStore.clear();
		screenStore.setActive('chat');
		drawerStore.close();
		goto('/', { replaceState: true, keepFocus: true, noScroll: true });
	}

	function handleCapabilitiesNav() {
		screenStore.setActive('capabilities');
		drawerStore.close();
	}

	function handleArtifactsNav() {
		screenStore.setActive('artifacts');
		drawerStore.close();
	}

	function handleInvokeCapability(cap: CapEntry) {
		screenStore.setActive('chat');
		const prompt = buildInvocationPrompt(cap);
		chatStream.send(prompt, {
			workspaceNodeId: selectedNodeId,
			forcedCapability: cap.name,
			onThreadId(id) { recentsStore.add(id); },
		});
	}

	// ── Keyboard shortcuts ───────────────────────────────────────────
	$effect(() => {
		return registerKeyboardShortcuts({
			onFocusComposer: () => composerRef?.focus(),
			onEscape: () => {
				if (drawerStore.open) { drawerStore.close(); return; }
				if (screenStore.active !== 'chat') screenStore.setActive('chat');
			},
			onCommandPalette: () => { /* future */ },
		});
	});

	// ── Cmd+N new chat ───────────────────────────────────────────────
	function onKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'n') { e.preventDefault(); handleNewChat(); }
	}

	// ── Restore state from URL params on mount ───────────────────────
	onMount(async () => {
		const route = await initialRoute();
		await applyInitialRoute<WorkspaceNode>(sdk, route, {
			onApplyNode(node) { onSelectNode(node); },
			onUnknown() {
				toasts.warning('Workspace not found, returning to root');
				goto('/', { replaceState: true, keepFocus: true, noScroll: true });
			},
		});
		if (route.cap) {
			screenStore.setActive('chat');
		}
	});

	// ── Workspace revalidation on resource_invalidated ───────────────
	let lastInvalidationKey = $state<string | null>(null);
	$effect(() => {
		const inv = chatStream.lastInvalidation;
		if (inv && inv.resource === 'workspace') {
			const key = JSON.stringify(inv);
			if (key !== lastInvalidationKey) {
				lastInvalidationKey = key;
				sdk.workspaces.tree().then(result => {
					if (!result.error && Array.isArray(result.data)) {
						workspaceNodes = result.data;
					}
				});
			}
		}
	});
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head><title>Workshop · ConusAI</title></svelte:head>

<!-- ── Shared sidebar content (rendered in shell sidebar + mobile drawer) ── -->
{#snippet navContent()}
	<WorkspaceExplorer
		{sdk}
		bind:nodes={workspaceNodes}
		bind:selectedNodeId
		{onSelectNode}
	/>

	<DrawerRecentChats
		{sdk}
		tenantId={data.user?.tenantId ?? null}
		{chatStream}
		onSelect={(thread) => {
			chatStream.loadThread(thread.id);
			breadcrumbsStore.clear();
			selectedNode = null;
			selectedNodeId = undefined;
			screenStore.setActive('chat');
			drawerStore.close();
		}}
	/>

	<SidebarSection>
		<ul role="list" class="nav-list">
			<SidebarItem
				icon={LayoutGrid}
				active={screenStore.active === 'capabilities'}
				onclick={handleCapabilitiesNav}
			>Capabilities</SidebarItem>
			<SidebarItem
				icon={FileText}
				active={screenStore.active === 'artifacts'}
				onclick={handleArtifactsNav}
			>Artifacts</SidebarItem>
		</ul>
	</SidebarSection>
{/snippet}

{#snippet userFooter()}
	<a href="/account" class="user-chip" aria-label="Open account settings">
		<div class="avatar" aria-hidden="true">
			{data.user?.initials ?? '?'}
		</div>
		<div class="user-meta">
			<span class="user-name">{data.user?.name ?? ''}</span>
			<span class="user-plan">{data.user?.plan ?? ''}</span>
		</div>
		<Icon icon={ArrowLeft} size="sm" class="user-chevron" />
	</a>
{/snippet}

<AppShell>
	<!-- ── Topbar ────────────────────────────────────────────────── -->
	{#snippet topbar()}
		<AppHeader>
			{#snippet leading()}
				<!-- Compact: hamburger. Medium+: back button or nothing. -->
				<button
					class="icon-btn shell-hamburger"
					aria-label="Open navigation"
					aria-expanded={drawerStore.open}
					onclick={() => drawerStore.toggle()}
				>
					<Icon icon={Menu} size="md" />
				</button>
				{#if screenStore.canGoBack}
					<button
						class="icon-btn"
						aria-label="Go back"
						onclick={() => screenStore.pop()}
					>
						<Icon icon={ArrowLeft} size="md" />
					</button>
				{/if}
			{/snippet}

			{#snippet title()}
				{screenTitle}
			{/snippet}

			{#snippet trailing()}
				{#if screenStore.active === 'chat'}
					<button
						class="icon-btn"
						aria-label="New conversation (⌘N)"
						title="New conversation (⌘N)"
						onclick={handleNewChat}
					>
						<Icon icon={Plus} size="md" />
					</button>
				{/if}

				<ThemeSwitcher />

				<a
					href="/logout"
					class="icon-btn"
					aria-label="Sign out"
					data-sveltekit-reload
				>
					<Icon icon={ArrowLeft} size="md" />
				</a>
			{/snippet}
		</AppHeader>
	{/snippet}

	<!-- ── Persistent sidebar (medium ≥768px) ────────────────────── -->
	{#snippet sidebar()}
		<Sidebar>
			{@render navContent()}
			{#snippet footer()}
				{@render userFooter()}
			{/snippet}
		</Sidebar>
	{/snippet}

	<!-- ── Main content ──────────────────────────────────────────── -->
	{#snippet main()}
		{#if screenStore.active === 'chat'}
			<ChatScreen
				{sdk}
				{chatStream}
				selectedNode={selectedNode}
				onSelectNode={(n) => {
					selectedNode = n;
					selectedNodeId = n?.id;
					breadcrumbsStore.set(n);
				}}
				userName={data.user?.name ?? 'there'}
				sigil={favicon}
			/>
		{:else if screenStore.active === 'capabilities'}
			<CapabilitiesScreen {sdk} onInvoke={handleInvokeCapability} />
		{:else if screenStore.active === 'artifacts'}
			<ArtifactsScreen {sdk} />
		{/if}
	{/snippet}
</AppShell>

<!-- ── Mobile drawer (compact <768px) ────────────────────────────────── -->
<Drawer
	open={drawerStore.open}
	onclose={() => drawerStore.close()}
	label="Navigation"
>
	<Sidebar>
		{@render navContent()}
		{#snippet footer()}
			{@render userFooter()}
		{/snippet}
	</Sidebar>
</Drawer>

<style>
	/* ── Icon button (topbar actions, hamburger) ──────────────────────────── */
	.icon-btn {
		display:         flex;
		align-items:     center;
		justify-content: center;
		width:           var(--hit, 44px);
		height:          var(--hit, 44px);
		border:          none;
		background:      transparent;
		color:           var(--color-fg-muted);
		cursor:          pointer;
		border-radius:   var(--radius-sm);
		text-decoration: none;
		padding:         0;
		transition:
			background var(--duration-fast) var(--ease-standard),
			color      var(--duration-fast) var(--ease-standard);
	}
	.icon-btn:hover {
		background: var(--color-bg-hover);
		color:      var(--color-fg);
	}
	.icon-btn:focus-visible {
		outline:        var(--focus-ring);
		outline-offset: var(--focus-ring-offset);
	}

	/* Hamburger hides on medium+ (sidebar is persistent there) */
	.shell-hamburger {
		display: flex;
	}
	@container app-shell (min-width: 768px) {
		.shell-hamburger {
			display: none;
		}
	}

	/* ── Nav list ─────────────────────────────────────────────────────────── */
	.nav-list {
		list-style: none;
		margin:     0;
		padding:    0;
	}

	/* ── User chip (sidebar/drawer footer) ────────────────────────────────── */
	.user-chip {
		display:        flex;
		align-items:    center;
		gap:            var(--space-2);
		padding:        var(--space-2) var(--space-3);
		text-decoration: none;
		color:          inherit;
		border-radius:  var(--radius-sm);
		min-height:     var(--hit, 44px);
		transition:     background var(--duration-fast) var(--ease-standard);
	}
	.user-chip:hover      { background: var(--color-bg-hover); }
	.user-chip:focus-visible {
		outline:        var(--focus-ring);
		outline-offset: var(--focus-ring-offset);
	}

	.avatar {
		width:           28px;
		height:          28px;
		border-radius:   50%;
		background:      var(--color-accent-soft);
		border:          1px solid var(--color-accent-border, var(--color-border));
		display:         flex;
		align-items:     center;
		justify-content: center;
		font-size:       var(--font-size-label);
		font-weight:     600;
		color:           var(--color-accent);
		flex-shrink:     0;
		user-select:     none;
	}

	.user-meta {
		display:        flex;
		flex-direction: column;
		flex:           1;
		min-width:      0;
	}
	.user-name {
		font-size:      var(--font-size-meta);
		color:          var(--color-fg);
		overflow:       hidden;
		text-overflow:  ellipsis;
		white-space:    nowrap;
	}
	.user-plan {
		font-size:      var(--font-size-label);
		color:          var(--color-fg-subtle);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	/* Hide user meta in icon-only rail (medium breakpoint, 768–1023px) */
	@container app-shell (max-width: 1023px) {
		.user-meta,
		.user-chevron { display: none; }
		.avatar       { margin: auto; }
	}

	@media (prefers-reduced-motion: reduce) {
		.icon-btn, .user-chip { transition: none; }
	}
</style>
