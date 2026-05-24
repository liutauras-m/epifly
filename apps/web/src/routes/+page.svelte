<svelte:options runes={true} />
<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import type { PageData } from './$types.js';
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
		WorkspaceTree,
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
		ThemeSwitcher,
		Icon,
		startViewTransition,
	} from '@conusai/ui';
	import { LayoutGrid, FileText, ArrowLeft, Plus, Search } from '@lucide/svelte';
	import { registerKeyboardShortcuts } from '@conusai/ui/utils/keyboard.js';
	import { t } from '@conusai/ui';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	// ── shadcn Sidebar ───────────────────────────────────────────────
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import { cn } from '$lib/utils.js';

	let { data }: { data: PageData } = $props();

	const registry = provideCapabilityRendererRegistry();
	const chatStream = createChatStream(sdk, { tenantId: data.user?.tenantId ?? null });

	// ── Workspace state ──────────────────────────────────────────────
	let workspaceNodes = $state<WorkspaceNode[]>(data.workspaceTree ?? []);
	let selectedNodeId = $state<string | undefined>();
	let selectedNode = $state<WorkspaceNode | null>(null);
	let composerRef = $state<{ focus(): void } | undefined>();

	// ── Derived screen title ─────────────────────────────────────────
	const screenTitle = $derived(
		screenStore.active === 'capabilities' ? 'Capabilities' :
		screenStore.active === 'artifacts'    ? 'Artifacts' :
		breadcrumbsStore.node?.name ?? 'Workshop'
	);

	// ── Navigation handlers ──────────────────────────────────────────
	function onSelectNode(node: WorkspaceNode) {
		selectedNode = node;
		selectedNodeId = node.id;
		breadcrumbsStore.set(node);
		recentsStore.add(node.id);
		if (node.kind === 'conversation') {
			if ((node.metadata as any)?.thread_id) {
				chatStream.loadThread((node.metadata as any).thread_id as string);
			} else {
				chatStream.newSession();
			}
		}
		startViewTransition(() => { screenStore.setActive('chat'); });
		drawerStore.close();
		goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true });
	}

	function handleNewChat() {
		chatStream.newSession();
		selectedNode = null;
		selectedNodeId = undefined;
		breadcrumbsStore.clear();
		startViewTransition(() => { screenStore.setActive('chat'); });
		drawerStore.close();
		goto('/', { replaceState: true, keepFocus: true, noScroll: true });
	}

	function handleCapabilitiesNav() {
		startViewTransition(() => { screenStore.setActive('capabilities'); });
		drawerStore.close();
	}

	function handleArtifactsNav() {
		startViewTransition(() => { screenStore.setActive('artifacts'); });
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

	function onKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'n') { e.preventDefault(); handleNewChat(); }
	}

	// ── Restore state from URL on mount ──────────────────────────────
	onMount(async () => {
		const route = await initialRoute();
		await applyInitialRoute<WorkspaceNode>(sdk, route, {
			onApplyNode(node) { onSelectNode(node); },
			onUnknown() {
				toasts.warning('Workspace not found, returning to root');
				goto('/', { replaceState: true, keepFocus: true, noScroll: true });
			},
		});
		if (route.cap) screenStore.setActive('chat');
	});

	// ── Workspace revalidation ───────────────────────────────────────
	let lastInvalidationKey = $state<string | null>(null);
	$effect(() => {
		const inv = chatStream.lastInvalidation;
		if (inv && inv.resource === 'workspace') {
			const key = JSON.stringify(inv);
			if (key !== lastInvalidationKey) {
				lastInvalidationKey = key;
				sdk.workspaces.tree().then((result: { error?: unknown; data?: unknown }) => {
					if (!result.error && Array.isArray(result.data)) {
						workspaceNodes = result.data as import('@conusai/types').WorkspaceNode[];
					}
				});
			}
		}
	});
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head><title>Workshop · ConusAI</title></svelte:head>

<!-- Skip navigation links (a11y) -->
<nav class="skip-links" aria-label="Skip navigation">
	<a class="skip-link" href="#main-content">{t('nav.skip_to_main')}</a>
	<a class="skip-link" href="#composer-input">{t('nav.skip_to_composer')}</a>
</nav>

<!-- ── shadcn Sidebar layout ──────────────────────────────────────────── -->
<Sidebar.Provider>

	<!-- ── Left nav sidebar ───────────────────────────────────────── -->
	<Sidebar.Root collapsible="icon" class="border-r border-sidebar-border">

		<!-- Header: workspace label + new-chat ─────────────────────── -->
		<Sidebar.Header class="border-b border-sidebar-border px-3 py-2">
			<Sidebar.Menu>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton size="lg" class="gap-2 hover:bg-sidebar-accent">
						{#snippet child({ props })}
							<button {...props} onclick={handleNewChat} aria-label={t('nav.new_chat')} title="{t('nav.new_chat')} (⌘N)">
								<div class="flex aspect-square size-8 items-center justify-center rounded-lg bg-[var(--color-accent)] text-white">
									<img src={favicon} alt="" class="size-5" />
								</div>
								<div class="flex flex-col gap-0.5 leading-none">
									<span class="text-sm font-semibold tracking-tight">Workshop</span>
									<span class="text-xs text-sidebar-foreground/60">ConusAI</span>
								</div>
							</button>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
			</Sidebar.Menu>
		</Sidebar.Header>

		<!-- Scrollable nav content ──────────────────────────────────── -->
		<Sidebar.Content class="gap-0">

			<!-- Workspace tree ──────────────────────────────────────── -->
			<Sidebar.Group class="p-0 group-data-[collapsible=icon]:hidden">
				<Sidebar.GroupLabel class="px-3 py-2 text-xs font-medium text-sidebar-foreground/60 uppercase tracking-widest">
					Workspace
				</Sidebar.GroupLabel>
				<Sidebar.GroupContent>
					<WorkspaceTree
						{sdk}
						bind:nodes={workspaceNodes}
						bind:selectedNodeId
						{onSelectNode}
					/>
				</Sidebar.GroupContent>
			</Sidebar.Group>

			<!-- Recent chats ────────────────────────────────────────── -->
			<Sidebar.Group class="p-0 group-data-[collapsible=icon]:hidden">
				<Sidebar.GroupContent>
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
				</Sidebar.GroupContent>
			</Sidebar.Group>

			<!-- Bottom nav: Capabilities + Artifacts ────────────────── -->
			<Sidebar.Group class="mt-auto border-t border-sidebar-border pt-2">
				<Sidebar.GroupContent>
					<Sidebar.Menu>
						<Sidebar.MenuItem>
							<Sidebar.MenuButton
								isActive={screenStore.active === 'capabilities'}
								onclick={handleCapabilitiesNav}
								tooltipContent="Capabilities"
							>
								<Icon icon={LayoutGrid} size="sm" />
								<span>Capabilities</span>
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
						<Sidebar.MenuItem>
							<Sidebar.MenuButton
								isActive={screenStore.active === 'artifacts'}
								onclick={handleArtifactsNav}
								tooltipContent="Artifacts"
							>
								<Icon icon={FileText} size="sm" />
								<span>Artifacts</span>
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					</Sidebar.Menu>
				</Sidebar.GroupContent>
			</Sidebar.Group>

		</Sidebar.Content>

		<!-- User footer ─────────────────────────────────────────────── -->
		<Sidebar.Footer class="border-t border-sidebar-border p-2">
			<Sidebar.Menu>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton size="lg" class="data-[state=open]:bg-sidebar-accent" tooltipContent={data.user?.name ?? ''}>
						{#snippet child({ props })}
							<a href="/account" {...props} aria-label={t('nav.account_settings')}>
								<div class="avatar" aria-hidden="true">
									{data.user?.initials ?? '?'}
								</div>
								<div class="flex flex-col leading-tight group-data-[collapsible=icon]:hidden">
									<span class="text-sm font-medium truncate">{data.user?.name ?? ''}</span>
									<span class="text-xs text-sidebar-foreground/60 uppercase tracking-wide">{data.user?.plan ?? ''}</span>
								</div>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
			</Sidebar.Menu>
		</Sidebar.Footer>

		<!-- Clickable rail (desktop collapse toggle) ─────────────────── -->
		<Sidebar.Rail />

	</Sidebar.Root>

	<!-- ── Main content area ──────────────────────────────────────── -->
	<Sidebar.Inset class="flex flex-col overflow-hidden">

		<!-- Topbar ──────────────────────────────────────────────────── -->
		<header class="app-header" data-tauri-drag-region>
			<!-- Safe-area fill for iOS -->
			<div class="header-safe" aria-hidden="true"></div>

			<div class="header-inner">
				<!-- Sidebar toggle (replaces hamburger on compact) -->
				<Sidebar.Trigger class="icon-btn" aria-label={t('nav.open_navigation')} />

				<!-- Screen title ─────────────────────────────────────── -->
				<span class="header-title">{screenTitle}</span>

				<!-- Trailing actions ────────────────────────────────── -->
				<div class="header-trailing">
					{#if screenStore.active === 'chat'}
						<button
							class="icon-btn"
							aria-label={t('nav.new_chat')}
							title="{t('nav.new_chat')} (⌘N)"
							onclick={handleNewChat}
						>
							<Icon icon={Plus} size="md" />
						</button>
					{/if}
					<ThemeSwitcher />
					<a href="/logout" class="icon-btn" aria-label={t('nav.sign_out')} data-sveltekit-reload>
						<Icon icon={ArrowLeft} size="md" />
					</a>
				</div>
			</div>
		</header>

		<!-- Main scrollable content ──────────────────────────────────── -->
		<main class="main-content" tabindex="-1" id="main-content">
			<div class="main-transition-root">
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

	</Sidebar.Inset>

</Sidebar.Provider>

<style>
	/* ── Skip links ───────────────────────────────────────────────────────── */
	.skip-links { position: absolute; }
	.skip-link {
		position: absolute;
		left: -9999px;
		top: var(--space-2, 8px);
		padding: var(--space-2, 8px) var(--space-3, 12px);
		background: var(--color-fg);
		color: var(--color-bg);
		border-radius: var(--radius-sm, 6px);
		font-size: var(--font-size-label, 13px);
		font-weight: 600;
		z-index: 9999;
		text-decoration: none;
	}
	.skip-link:focus { left: var(--space-2, 8px); }

	/* ── Header ──────────────────────────────────────────────────────────── */
	.app-header {
		background:    var(--color-bg);
		border-bottom: 1px solid var(--color-border);
		flex-shrink:   0;
		z-index:       var(--z-topbar, 100);
		padding-left:  env(titlebar-area-inset-left, 0px);
	}

	.header-safe {
		height:     var(--safe-top, 0px);
		background: inherit;
	}

	.header-inner {
		display:     flex;
		align-items: center;
		height:      var(--topbar-height, 56px);
		padding:     0 var(--space-2, 8px);
		gap:         var(--space-1, 4px);
	}

	.header-title {
		flex:            1;
		min-width:       0;
		overflow:        hidden;
		text-overflow:   ellipsis;
		white-space:     nowrap;
		text-align:      center;
		font-family:     var(--font-family-sans);
		font-size:       var(--font-size-h2, 20px);
		font-weight:     580;
		letter-spacing:  -0.018em;
		color:           var(--color-fg);
	}

	.header-trailing {
		display:         flex;
		align-items:     center;
		gap:             var(--space-1, 4px);
		flex-shrink:     0;
	}

	/* ── Icon buttons ─────────────────────────────────────────────────────── */
	:global(.icon-btn) {
		display:         flex;
		align-items:     center;
		justify-content: center;
		width:           var(--hit, 44px);
		height:          var(--hit, 44px);
		border:          none;
		background:      transparent;
		color:           var(--color-fg-muted);
		cursor:          pointer;
		border-radius:   var(--radius-sm, 6px);
		text-decoration: none;
		padding:         0;
		transition:
			background var(--duration-fast, 120ms) var(--ease-standard, ease),
			color      var(--duration-fast, 120ms) var(--ease-standard, ease);
	}
	:global(.icon-btn:hover)         { background: var(--color-bg-hover); color: var(--color-fg); }
	:global(.icon-btn:focus-visible)  { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset, 2px); }

	/* ── Main content ─────────────────────────────────────────────────────── */
	.main-content {
		flex:       1;
		min-height: 0;
		overflow:   auto;
		display:    flex;
		flex-direction: column;
	}

	/* ── Avatar chip ──────────────────────────────────────────────────────── */
	:global(.avatar) {
		width:           2rem;
		height:          2rem;
		border-radius:   50%;
		background:      var(--color-accent-soft, #fff1e6);
		border:          1px solid var(--color-accent-border, var(--color-border));
		display:         flex;
		align-items:     center;
		justify-content: center;
		font-size:       var(--font-size-label, 13px);
		font-weight:     600;
		color:           var(--color-accent, #ff6200);
		flex-shrink:     0;
		user-select:     none;
	}

	/* ── View transition [continuity] ────────────────────────────────────── */
	.main-transition-root {
		display:              contents;
		view-transition-name: main-content;
		contain:              layout;
	}

	:global(::view-transition-old(main-content)) {
		animation: vt-fade-out var(--duration-normal, 200ms) var(--ease-standard, ease) both;
	}
	:global(::view-transition-new(main-content)) {
		animation: vt-fade-in  var(--duration-normal, 200ms) var(--ease-standard, ease) both;
	}

	@keyframes vt-fade-out { from { opacity: 1; transform: translateY(0);   } to { opacity: 0; transform: translateY(-6px); } }
	@keyframes vt-fade-in  { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0);   } }

	@media (prefers-reduced-motion: reduce) {
		:global(::view-transition-old(main-content)) { animation-duration: 80ms !important; animation-name: vt-opacity-out !important; }
		:global(::view-transition-new(main-content)) { animation-duration: 80ms !important; animation-name: vt-opacity-in  !important; }
		@keyframes vt-opacity-out { from { opacity: 1; } to { opacity: 0; } }
		@keyframes vt-opacity-in  { from { opacity: 0; } to { opacity: 1; } }
		:global(.icon-btn) { transition: none; }
	}
</style>
