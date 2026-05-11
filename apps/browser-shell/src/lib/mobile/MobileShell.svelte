<script lang="ts">
	import { onMount } from 'svelte';
	import { sdk, setSessionToken, getSessionToken } from '$lib/sdk';
	import { isTauri, streamChatTauri } from '$lib/tauri-stream';
	import { createChatStream } from '@conusai/ui/features';
	import type { WorkspaceNode } from '@conusai/types';

	import { setPlatformTag } from './platform/detect.js';
	import { tap } from '@conusai/ui/motion';
	import { screenStore } from './stores/screen.svelte.js';
	import { drawerStore } from './stores/drawer.svelte.js';
	import { sheetStore } from './stores/sheet.svelte.js';
	import { recentsStore, breadcrumbsStore } from '@conusai/ui/stores';

	import MobileTopBar from './chrome/MobileTopBar.svelte';
	import MobileDrawer from './chrome/MobileDrawer.svelte';

	import DrawerProfileHeader from './parts/DrawerProfileHeader.svelte';
	import DrawerWorkspaceTree from './parts/DrawerWorkspaceTree.svelte';
	import DrawerRecentChats from './parts/DrawerRecentChats.svelte';
	import ProfileSheet from './parts/ProfileSheet.svelte';

	import ChatScreen from './screens/ChatScreen.svelte';
	import CapabilitiesScreen from './screens/CapabilitiesScreen.svelte';
	import ArtifactsScreen from './screens/ArtifactsScreen.svelte';
	import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';

	// ── Auth state ──────────────────────────────────────────────────
	let user = $state<{ name: string; plan: string } | null>(null);
	let nameInput = $state('');
	let planInput = $state('enterprise');
	let nameError = $state('');

	// ── Workspace state ──────────────────────────────────────────────
	let workspaceNodes = $state<WorkspaceNode[]>([]);
	let selectedNode = $state<WorkspaceNode | null>(null);

	// ── Profile sheet ────────────────────────────────────────────────
	let profileSheetOpen = $state(false);

	// ── Tauri streaming ──────────────────────────────────────────────
	const tauriStreamFn = isTauri
		? (params: import('@conusai/sdk').StreamChatParams) =>
				streamChatTauri({
					message: params.message,
					sessionToken: getSessionToken() ?? '',
					threadId: params.threadId,
					workspaceNodeId: params.workspaceNodeId,
					attachmentIds: params.attachmentIds,
					signal: params.signal,
				})
		: undefined;

	const chatStream = createChatStream(sdk, { streamFn: tauriStreamFn });

	// ── Screen title ─────────────────────────────────────────────────
	const screenTitle = $derived(
		screenStore.active === 'capabilities' ? 'Capabilities' :
		screenStore.active === 'artifacts' ? 'Artifacts' :
		breadcrumbsStore.node?.name ?? 'ConusAI'
	);

	// ── Lifecycle ────────────────────────────────────────────────────
	onMount(() => {
		setPlatformTag();

		// Restore cached token sync so API calls work immediately
		const cachedToken = localStorage.getItem('conusai_shell_token');
		if (cachedToken) setSessionToken(cachedToken);

		// Restore user
		const raw = localStorage.getItem('conusai_shell_user');
		if (raw) {
			try {
				user = JSON.parse(raw);
				if (user) issueSessionCookie(user.name, user.plan).catch(() => {});
			} catch { /* corrupt */ }
		}
	});

	// ── Auth ─────────────────────────────────────────────────────────
	async function issueSessionCookie(name: string, plan: string) {
		const UI_SESSION_KEY = import.meta.env.VITE_UI_SESSION_KEY ?? 'conusai-foundry-dev-secret-change-me-32b';
		const exp = Math.floor(Date.now() / 1000) + 7 * 86_400;
		const payload = JSON.stringify({ name, plan, role: 'user', exp });
		const payloadB64 = btoa(payload).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
		const key = await crypto.subtle.importKey(
			'raw', new TextEncoder().encode(UI_SESSION_KEY),
			{ name: 'HMAC', hash: 'SHA-256' }, false, ['sign'],
		);
		const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(payloadB64));
		const mac = btoa(String.fromCharCode(...new Uint8Array(sig))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
		const token = `${payloadB64}.${mac}`;
		setSessionToken(token);
		localStorage.setItem('conusai_shell_token', token);
		const apiBase = import.meta.env.VITE_API_BASE ?? '';
		const domain = apiBase ? new URL(apiBase).hostname : 'localhost';
		document.cookie = `conusai_session=${token}; path=/; domain=${domain}; SameSite=Lax`;
	}

	function handleBegin() {
		const name = nameInput.trim();
		if (!name || name.length > 60) { nameError = 'Name must be 1–60 characters.'; return; }
		nameError = '';
		user = { name, plan: planInput };
		localStorage.setItem('conusai_shell_user', JSON.stringify(user));
		issueSessionCookie(name, planInput).catch(() => {});
	}

	function handleLogout() {
		localStorage.removeItem('conusai_shell_user');
		localStorage.removeItem('conusai_shell_token');
		setSessionToken(null);
		user = null;
		nameInput = '';
		planInput = 'enterprise';
		chatStream.newSession();
		breadcrumbsStore.clear();
		recentsStore.clear();
		drawerStore.close();
	}

	// ── Navigation ───────────────────────────────────────────────────
	function handleSelectNode(node: WorkspaceNode) {
		selectedNode = node;
		breadcrumbsStore.set(node);
		drawerStore.close();
		screenStore.setActive('chat');
		if (node.kind === 'conversation' && (node as any).metadata?.thread_id) {
			chatStream.loadThread((node as any).metadata.thread_id);
		}
		recentsStore.add(node.id);
	}

	function handleNewChat() {
		selectedNode = null;
		breadcrumbsStore.clear();
		chatStream.newSession();
		drawerStore.close();
		screenStore.setActive('chat');
	}

	function handleCapabilitiesNav() {
		screenStore.setActive('capabilities');
		drawerStore.close();
	}

	function handleArtifactsNav() {
		screenStore.setActive('artifacts');
		drawerStore.close();
	}

	function handleInvoke(capName: string) {
		screenStore.setActive('chat');
		// prefill handled by ChatScreen via suggestion; here just switch screen
	}
</script>

{#if !user}
	<!-- ── Login screen ──────────────────────────────────────────── -->
	<div class="login-screen">
		<div class="login-card" role="main">
			<div class="login-brand">
				<img class="brand-logo" src={logoDark} alt="ConusAI" />
				<span class="brand-name">ConusAI</span>
			</div>

			<div class="login-copy">
				<h1 class="login-title">Enter the workshop.</h1>
				<p class="login-sub">An agent platform built for operators who build with intent.</p>
			</div>

			<form class="login-form" onsubmit={(e) => { e.preventDefault(); handleBegin(); }}>
				<div class="field">
					<label class="field-label" for="shell-name-input">Your name</label>
					<input
						id="shell-name-input"
						class="field-input"
						class:error={!!nameError}
						type="text"
						bind:value={nameInput}
						placeholder="John Smith"
						maxlength="60"
						autocomplete="off"
						autocorrect="off"
						autocapitalize="words"
						spellcheck={false}
						required
					/>
					{#if nameError}<p class="field-error" role="alert">{nameError}</p>{/if}
				</div>

				<fieldset class="plan-fieldset">
					<legend class="field-label">Plan tier</legend>
					<div class="plan-row">
						{#each [['free', 'Free'], ['pro', 'Pro'], ['enterprise', 'Enterprise']] as [val, label]}
							<label class="plan-option" class:selected={planInput === val}>
								<input type="radio" name="shell-plan" value={val} bind:group={planInput} />
								<span class="plan-label">{label}</span>
							</label>
						{/each}
					</div>
				</fieldset>

				<button class="begin-btn" type="submit">
					Get started
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18">
						<path d="M5 12h14M12 5l7 7-7 7"/>
					</svg>
				</button>
			</form>
		</div>
	</div>

{:else}
	<!-- ── Main shell ────────────────────────────────────────────── -->
	<div class="shell">
		<MobileTopBar
			onMenuToggle={() => drawerStore.toggle()}
			canGoBack={screenStore.canGoBack}
			onBack={() => screenStore.pop()}
			title={screenTitle}
		>
			{#snippet rightAction()}
				{#if screenStore.active === 'chat'}
					<button class="topbar-icon-btn" aria-label="New conversation" onclick={handleNewChat}>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
							<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
							<line x1="12" y1="9" x2="12" y2="13"/>
							<line x1="10" y1="11" x2="14" y2="11"/>
						</svg>
					</button>
				{/if}
			{/snippet}
		</MobileTopBar>

		<div class="screen-host">
			{#if screenStore.active === 'chat'}
				<ChatScreen
					{sdk}
					{chatStream}
					selectedNode={selectedNode}
					onSelectNode={(n) => { selectedNode = n; breadcrumbsStore.set(n); }}
					userName={user.name}
				/>
			{:else if screenStore.active === 'capabilities'}
				<CapabilitiesScreen {sdk} onInvoke={handleInvoke} />
			{:else if screenStore.active === 'artifacts'}
				<ArtifactsScreen {sdk} />
			{/if}
		</div>

		<MobileDrawer open={drawerStore.open} onClose={() => drawerStore.close()}>
			{#snippet children()}
				<DrawerProfileHeader
					name={user.name}
					plan={user.plan}
					onOpenProfile={() => profileSheetOpen = true}
				/>

				<DrawerWorkspaceTree
					{sdk}
					selectedNodeId={selectedNode?.id}
					onSelectNode={handleSelectNode}
				/>

				<DrawerRecentChats
					recentIds={recentsStore.ids}
					nodes={workspaceNodes}
					onSelect={handleSelectNode}
				/>

				<!-- Secondary links -->
				<div class="drawer-links">
					<button class="drawer-link" use:tap onclick={handleCapabilitiesNav}>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
							<rect x="2" y="3" width="6" height="6" rx="1"/>
							<rect x="16" y="3" width="6" height="6" rx="1"/>
							<rect x="2" y="15" width="6" height="6" rx="1"/>
							<rect x="16" y="15" width="6" height="6" rx="1"/>
							<line x1="8" y1="6" x2="16" y2="6"/>
							<line x1="8" y1="18" x2="16" y2="18"/>
							<line x1="5" y1="9" x2="5" y2="15"/>
							<line x1="19" y1="9" x2="19" y2="15"/>
						</svg>
						Capabilities
					</button>
					<button class="drawer-link" use:tap onclick={handleArtifactsNav}>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
							<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
							<polyline points="13 2 13 9 20 9"/>
						</svg>
						Artifacts
					</button>
				</div>
			{/snippet}
		</MobileDrawer>
	</div>

	<ProfileSheet
		open={profileSheetOpen}
		name={user.name}
		plan={user.plan}
		onClose={() => profileSheetOpen = false}
		onLogout={handleLogout}
	/>
{/if}

<style>
	/* ── Login ── */
	.login-screen {
		min-height: 100dvh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--paper);
		padding: var(--s-4);
	}

	.login-card {
		width: 100%;
		max-width: 440px;
		display: flex;
		flex-direction: column;
		gap: var(--s-5);
	}

	.login-brand {
		display: flex;
		align-items: center;
		gap: var(--s-2);
	}

	.brand-logo {
		height: 28px;
		width: auto;
		display: block;
	}

	.brand-name {
		font-family: var(--font-display);
		font-size: 20px;
		font-weight: 700;
		color: var(--ink);
	}

	.login-title {
		font-family: var(--font-display);
		font-size: 32px;
		font-weight: 700;
		letter-spacing: -1px;
		line-height: 1.1;
		color: var(--ink);
		margin: 0;
	}

	.login-sub {
		font-family: var(--font-body);
		font-size: 16px;
		color: var(--ink-2);
		margin: var(--s-2) 0 0;
		line-height: 1.5;
	}

	.login-copy { display: flex; flex-direction: column; }

	.login-form { display: flex; flex-direction: column; gap: var(--s-4); }

	.field { display: flex; flex-direction: column; gap: var(--s-1); }

	.field-label {
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 500;
		letter-spacing: 0.08em;
		color: var(--ink-3);
		text-transform: uppercase;
	}

	.field-input {
		height: 48px;
		border: 1px solid var(--rule);
		border-radius: var(--r-md);
		padding: 0 var(--s-4);
		background: var(--paper-2);
		color: var(--ink);
		font-family: var(--font-body);
		font-size: 16px;
	}

	.field-input:focus { outline: none; border-color: var(--ember); }
	.field-input.error { border-color: var(--danger); }

	.field-error {
		font-family: var(--font-body);
		font-size: 13px;
		color: var(--danger);
		margin: 0;
	}

	.plan-fieldset {
		border: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
	}

	.plan-row { display: flex; gap: var(--s-2); }

	.plan-option {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		height: 44px;
		border: 1px solid var(--rule);
		border-radius: var(--r-md);
		cursor: pointer;
		transition: border-color 120ms, background 120ms;
	}

	.plan-option.selected {
		border-color: var(--ember);
		background: var(--ember-soft);
	}

	.plan-option input { display: none; }

	.plan-label {
		font-family: var(--font-body);
		font-size: 14px;
		color: var(--ink-2);
	}

	.plan-option.selected .plan-label { color: var(--ink); font-weight: 600; }

	.begin-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--s-2);
		height: 52px;
		background: var(--ember);
		color: var(--ink);
		border: none;
		border-radius: var(--r-md);
		font-family: var(--font-body);
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		transition: background 120ms;
	}

	.begin-btn:hover { background: var(--ember-2); }

	/* ── Shell ── */
	.shell {
		height: 100dvh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--paper);
		position: relative;
	}

	.screen-host {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		max-width: 880px;
		width: 100%;
		margin: 0 auto;
		align-self: stretch;
	}

	/* Desktop: wider content */
	@media (min-width: 641px) {
		.screen-host {
			max-width: 760px;
		}
	}

	/* ── Top bar actions ── */
	.topbar-icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 44px;
		height: 44px;
		border: none;
		background: none;
		color: var(--ink);
		cursor: pointer;
		border-radius: var(--r-sm);
	}

	.topbar-icon-btn:hover { background: var(--paper-2); }

	/* ── Drawer secondary links ── */
	.drawer-links {
		border-top: 1px solid var(--rule);
		display: flex;
		flex-direction: column;
		margin-top: auto;
	}

	.drawer-link {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		height: 48px;
		padding: 0 var(--s-4);
		border: none;
		background: none;
		font-family: var(--font-body);
		font-size: 15px;
		color: var(--ink-2);
		cursor: pointer;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--rule);
		transition: background 120ms;
	}

	.drawer-link:hover { background: var(--paper-3); color: var(--ink); }
</style>
