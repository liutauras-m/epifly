<script lang="ts">
	import { goto, afterNavigate } from '$app/navigation';
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
		AppTopBar,
		AppDrawer,
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
	import { ThemeSwitcher } from '@conusai/ui';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	let { data }: { data: PageData } = $props();

	const registry = provideCapabilityRendererRegistry();
	const chatStream = createChatStream(sdk, { tenantId: data.user?.tenantId ?? null });

	// ── Workspace state ──────────────────────────────────────────────
	let workspaceNodes = $state<WorkspaceNode[]>(data.workspaceTree ?? []);
	let selectedNodeId = $state<string | undefined>();
	let selectedNode = $state<WorkspaceNode | null>(null);

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
		// PR 2.A: pass forced_capability as structured data so the gateway pins
		// those tools before semantic routing. The prompt is now a brief natural
		// description — the semantic weight is carried by the structured hint.
		const prompt = buildInvocationPrompt(cap);
		chatStream.send(prompt, {
			workspaceNodeId: selectedNodeId,
			forcedCapability: cap.name,
			onThreadId(id) { recentsStore.add(id); },
		});
	}

	// ── Keyboard shortcuts ───────────────────────────────────────────
	function onKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'n') { e.preventDefault(); handleNewChat(); }
		if (e.key === 'Escape') {
			if (screenStore.active !== 'chat') screenStore.setActive('chat');
			else drawerStore.close();
		}
	}

	// ── Restore state from URL params on mount (PR 3.C / 3.C.5) ─────
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
			// Pre-select a capability for the first send
			screenStore.setActive('chat');
		}
	});

	// ── Workspace revalidation on resource_invalidated (PR 3.A) ─────
	// When the agent writes workspace artifacts, re-fetch the workspace tree
	// so WorkspaceExplorer reflects the new files without a manual refresh.
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

<div class="app">
	<!-- ── Drawer / Sidebar (responsive — overlay on mobile, persistent on desktop) ── -->
	<AppDrawer open={drawerStore.open} onClose={() => drawerStore.close()}>
		{#snippet children()}
			<WorkspaceExplorer
				{sdk}
				bind:nodes={workspaceNodes}
				bind:selectedNodeId
				{onSelectNode}
			/>

			<!-- Recent chats (live; refreshes on `resource_invalidated: "threads"`) -->
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

			<!-- Secondary nav: Capabilities, Artifacts -->
			<div class="drawer-links">
				<button
					class="drawer-link"
					class:active={screenStore.active === 'capabilities'}
					onclick={handleCapabilitiesNav}
				>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
						stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
						width="18" height="18" aria-hidden="true">
						<rect x="2" y="3" width="6" height="6" rx="1"/>
						<rect x="16" y="3" width="6" height="6" rx="1"/>
						<rect x="2" y="15" width="6" height="6" rx="1"/>
						<rect x="16" y="15" width="6" height="6" rx="1"/>
					</svg>
					Capabilities
				</button>
				<button
					class="drawer-link"
					class:active={screenStore.active === 'artifacts'}
					onclick={handleArtifactsNav}
				>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
						stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
						width="18" height="18" aria-hidden="true">
						<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
						<polyline points="13 2 13 9 20 9"/>
					</svg>
					Artifacts
				</button>
			</div>

			<!-- User chip — clickable, links to account -->
			<a href="/account" class="user-chip" aria-label="Open account settings">
				<div class="avatar">{data.user?.initials ?? '?'}</div>
				<div class="user-meta">
					<span class="user-name">{data.user?.name ?? ''}</span>
					<span class="user-plan">{data.user?.plan ?? ''}</span>
				</div>
				<svg class="user-chip-chevron" viewBox="0 0 16 16" fill="none"
					stroke="currentColor" stroke-width="1.5" stroke-linecap="round"
					width="12" height="12" aria-hidden="true">
					<path d="M6 4l4 4-4 4"/>
				</svg>
			</a>
		{/snippet}
	</AppDrawer>

	<!-- ── Main ─────────────────────────────────────────────────────── -->
	<main class="main">
		<AppTopBar
			onMenuToggle={() => drawerStore.toggle()}
			canGoBack={screenStore.canGoBack}
			onBack={() => screenStore.pop()}
			title={screenTitle}
		>
			{#snippet rightAction()}
				{#if screenStore.active === 'chat'}
					<button
						class="topbar-action"
						aria-label="New conversation"
						title="New conversation (⌘N)"
						onclick={handleNewChat}
					>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
							stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
							width="20" height="20" aria-hidden="true">
							<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
							<line x1="12" y1="9" x2="12" y2="13"/>
							<line x1="10" y1="11" x2="14" y2="11"/>
						</svg>
					</button>
				{/if}

				<ThemeSwitcher />

				<a
					href="/logout"
					class="topbar-action"
					aria-label="Sign out"
					data-sveltekit-reload
				>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
						stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
						width="20" height="20" aria-hidden="true">
						<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/>
						<polyline points="16 17 21 12 16 7"/>
						<line x1="21" y1="12" x2="9" y2="12"/>
					</svg>
				</a>
			{/snippet}
		</AppTopBar>

		<!-- Screen host -->
		<div class="screen-host">
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
		</div>
	</main>
</div>

<style>
	/* ── Layout ─────────────────────────────────────────────────────────── */
	.app {
		display: flex;
		height: 100dvh;
		overflow: hidden;
		background: var(--paper);
	}

	.main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}

	.screen-host {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	/* ── Topbar actions ─────────────────────────────────────────────────── */
	.topbar-action {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border: none;
		background: none;
		color: var(--ink-3);
		cursor: pointer;
		border-radius: var(--radius-sm);
		text-decoration: none;
		transition: background var(--duration-fast), color var(--duration-fast);
	}
	.topbar-action:hover { background: var(--paper-3); color: var(--ink); }
	.topbar-action:focus-visible { outline: 2px solid var(--ember); outline-offset: 2px; }

	/* ── Drawer / sidebar content ──────────────────────────────────────── */
	.drawer-links {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--rule);
		padding: var(--space-2) 0;
	}

	.drawer-link {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		height: 40px;
		padding: 0 var(--space-4);
		border: none;
		background: none;
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		color: var(--ink-2);
		cursor: pointer;
		width: 100%;
		text-align: left;
		transition: background var(--duration-fast), color var(--duration-fast);
	}
	.drawer-link:hover { background: var(--paper-3); color: var(--ink); }
	.drawer-link.active {
		background: var(--ember-soft);
		color: var(--ink);
		font-weight: 500;
	}
	.drawer-link:focus-visible {
		outline: 2px solid var(--ember);
		outline-offset: -2px;
	}

	/* ── User chip ─────────────────────────────────────────────────────── */
	.user-chip {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
		border-top: 1px solid var(--rule);
		margin-top: auto;
		text-decoration: none;
		color: inherit;
		transition: background var(--duration-fast);
	}
	.user-chip:hover { background: var(--paper-3); }
	.user-chip:focus-visible { outline: 2px solid var(--ember); outline-offset: -2px; }

	.avatar {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--ember-soft);
		border: 1px solid var(--ember-glow);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: var(--font-size-label);
		font-weight: 600;
		color: var(--ember-2);
		flex-shrink: 0;
	}
	.user-meta {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}
	.user-name {
		font-size: var(--font-size-meta);
		color: var(--ink);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.user-plan {
		font-size: var(--font-size-label);
		color: var(--ink-3);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.user-chip-chevron {
		color: var(--ink-3);
		flex-shrink: 0;
	}

	@media (prefers-reduced-motion: reduce) {
		.topbar-action, .drawer-link, .user-chip { transition: none; }
	}
</style>
