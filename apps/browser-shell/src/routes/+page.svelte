<script lang="ts">
	import { sdk, setSessionToken, getSessionToken } from '$lib/sdk';
	import { isTauri, streamChatTauri } from '$lib/tauri-stream';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { AgentChatStream, AgentChatComposer, WorkspaceExplorer, createChatStream, type Attachment } from '@conusai/ui/features';
	import { modeStore } from '@conusai/ui/stores';
	import TraceReplayCapability from '$lib/TraceReplayCapability.svelte';
	import type { WorkspaceNode } from '@conusai/types';
	import { onMount } from 'svelte';


	modeStore.setMode('shell');

	const registry = provideCapabilityRendererRegistry();
	registry.register('trace.replay', TraceReplayCapability as any);

	// On iOS WKWebView, fetch() buffers SSE responses before JS sees them.
	// Route streaming through Rust IPC instead.
	const tauriStreamFn = isTauri
		? (params: import('@conusai/sdk').StreamChatParams) =>
				streamChatTauri({
					message: params.message,
					sessionToken: getSessionToken() ?? '',
					threadId: params.threadId,
					workspaceNodeId: params.workspaceNodeId,
					signal: params.signal,
				})
		: undefined;
	const chatStream = createChatStream(sdk, { streamFn: tauriStreamFn });

	let user = $state<{ name: string; plan: string } | null>(null);
	let nameInput = $state('');
	let planInput = $state('enterprise');
	let nameError = $state('');

	let showChat = $state(false);
	let inputValue = $state('');
	let workspaceNodes = $state<WorkspaceNode[]>([]);
	let selectedNodeId = $state<string | undefined>();
	let sidebarOpen = $state(false);
	let caps = $state<{ name: string; kind: string }[]>([]);
	let recentNodeIds = $state<string[]>([]);
	let isMobile = $state(false);

	const recentConversations = $derived(
		recentNodeIds
			.map(id => workspaceNodes.find(n => n.id === id))
			.filter(Boolean) as import('@conusai/types').WorkspaceNode[]
	);

	const SUGGESTIONS = [
		'What can you help me with?',
		'Explain the difference between AI agents and AI assistants.',
		'What tools and capabilities do you have?',
		'What is the current time?',
	];

	onMount(() => {
		// Restore cached token immediately (sync) so API calls work before crypto.subtle completes.
		const cachedToken = localStorage.getItem('conusai_shell_token');
		if (cachedToken) setSessionToken(cachedToken);

		const raw = localStorage.getItem('conusai_shell_user');
		if (raw) {
			try {
				user = JSON.parse(raw);
				if (user) issueSessionCookie(user.name, user.plan).catch(() => {});
			} catch { /* corrupt — re-login */ }
		}
		const savedRecents = localStorage.getItem('conusai_recents');
		if (savedRecents) try { recentNodeIds = JSON.parse(savedRecents); } catch { /* ignore */ }
		fetchCaps();

		// Detect mobile — full-screen nav instead of CSS overlay
		const mql = window.matchMedia('(max-width: 640px)');
		isMobile = mql.matches;
		mql.addEventListener('change', e => { isMobile = e.matches; });
	});

	async function fetchCaps() {
		const res = await sdk.capabilities.list();
		if (!res.error && res.data) {
			const d = res.data as unknown as { capabilities?: { name: string; kind: string }[] } | { name: string; kind: string }[];
			caps = Array.isArray(d) ? d : (d as { capabilities?: { name: string; kind: string }[] }).capabilities ?? [];
		}
	}

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
		if (!name || name.length > 60) {
			nameError = 'Name must be 1–60 characters.';
			return;
		}
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
	}

	function handleSelectNode(node: WorkspaceNode) {
		showChat = true;
		sidebarOpen = false;
		selectedNodeId = node.id;
		if (node.kind === 'conversation') {
			if (node.metadata?.thread_id) chatStream.loadThread(node.metadata.thread_id as string);
			// Track in recents (deduplicated, max 8)
			recentNodeIds = [node.id, ...recentNodeIds.filter(id => id !== node.id)].slice(0, 8);
			localStorage.setItem('conusai_recents', JSON.stringify(recentNodeIds));
		}
	}

	function handleSubmit(prompt: string, attachments: Attachment[] = []) {
		if (!prompt.trim() && attachments.length === 0) return;
		showChat = true;
		chatStream.send(prompt, {
			workspaceNodeId: selectedNodeId,
			attachmentIds: attachments.map(a => a.id),
		});
	}

	async function handleUpload(files: File[]): Promise<Attachment[]> {
		const results: Attachment[] = [];
		for (const file of files) {
			const res = await sdk.files.upload(file);
			if (res.data) {
				results.push({
					id: res.data.token,
					filename: res.data.name,
					size: res.data.size_bytes,
				});
			}
		}
		return results;
	}

	function initials(name: string) {
		return name.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
	}

	function handleSuggestion(text: string) {
		inputValue = text;
		handleSubmit(text);
	}
</script>

{#if !user}
	<!-- ── Login screen ──────────────────────────────────────────── -->
	<div class="login-screen">
		<div class="login-card" role="main">
			<div class="login-brand">
				<div class="brand-mark">C</div>
				<span class="brand-name">ConusAI</span>
			</div>

			<div class="login-copy">
				<h1 class="login-title">Enter the workshop.</h1>
				<p class="login-sub">An agent platform built for operators who build with intent.</p>
			</div>

			<form class="login-form" onsubmit={(e) => { e.preventDefault(); handleBegin(); }}>
				<div class="field">
					<label class="field-label" for="name-input">Your name</label>
					<input
						id="name-input"
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
					{#if nameError}
						<p class="field-error" role="alert">{nameError}</p>
					{/if}
				</div>

				<fieldset class="plan-fieldset">
					<legend class="field-label">Plan tier</legend>
					<div class="plan-row">
						{#each [['free', 'Free'], ['pro', 'Pro'], ['enterprise', 'Enterprise']] as [val, label]}
							<label class="plan-option">
								<input type="radio" name="plan" value={val} bind:group={planInput} />
								<span class="plan-label">{label}</span>
							</label>
						{/each}
					</div>
				</fieldset>

				<button class="begin-btn" type="submit">
					Get started
					<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="18" height="18">
						<path d="M4 10h12M10 4l6 6-6 6"/>
					</svg>
				</button>
			</form>
		</div>
	</div>

{:else}
	<!-- ── Workspace ─────────────────────────────────────────────── -->
	<div class="workspace">
		<!-- On mobile: full-screen nav replaces main content (avoids position:fixed WKWebView bugs) -->
		<!-- On desktop: sidebar is always a flex column beside main content -->
		<aside class="sidebar" class:open={sidebarOpen} aria-label="Workspace navigation">
			<div class="sidebar-header">
				<div class="avatar">{initials(user.name)}</div>
				<div class="user-meta">
					<span class="user-name">{user.name}</span>
					<span class="user-plan">{user.plan}</span>
				</div>
				<button class="icon-btn logout-btn" aria-label="Sign out" onclick={handleLogout}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="17" height="17">
						<path d="M7 3H3v12h4M12 6l4 3-4 3M7 9h9"/>
					</svg>
				</button>
				<button class="icon-btn close-btn" aria-label="Close" onclick={() => (sidebarOpen = false)}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="18" height="18">
						<path d="M4 4l10 10M14 4L4 14"/>
					</svg>
				</button>
			</div>
			<div class="sidebar-body">
				<div class="sidebar-section sidebar-workspace">
					<WorkspaceExplorer {sdk} bind:nodes={workspaceNodes} bind:selectedNodeId onSelectNode={handleSelectNode} />
				</div>

				{#if recentConversations.length > 0}
					<div class="sidebar-section">
						<div class="section-label">Recents</div>
						{#each recentConversations as conv (conv.id)}
							<button
								class="sidebar-item"
								class:active={selectedNodeId === conv.id}
								onclick={() => handleSelectNode(conv)}
							>
								<svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" width="12" height="12" style="flex-shrink:0;opacity:0.5"><path d="M7 1a6 6 0 1 1-3.87 10.61L1 13l1.39-2.13A6 6 0 0 1 7 1z"/></svg>
								<span class="sidebar-item-name">{conv.name.replace(/\.md$/, '')}</span>
							</button>
						{/each}
					</div>
				{/if}

				{#if caps.length > 0}
					<div class="sidebar-section">
						<div class="section-label">Capabilities</div>
						{#each caps as cap (cap.name)}
							<div class="sidebar-item sidebar-cap">
								<span class="cap-kind">{cap.kind.toLowerCase().replace('remotemcp','mcp')}</span>
								<span class="sidebar-item-name">{cap.name}</span>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</aside>

		<div class="main-col" class:hidden={sidebarOpen}>
			<header class="topbar">
				<button class="icon-btn" aria-label="Open navigation" onclick={() => (sidebarOpen = !sidebarOpen)}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" width="18" height="18">
						<line x1="3" y1="5" x2="15" y2="5" stroke-linecap="square"/>
						<line x1="3" y1="9" x2="12" y2="9" stroke-linecap="square"/>
						<line x1="3" y1="13" x2="15" y2="13" stroke-linecap="square"/>
					</svg>
				</button>

				<span class="topbar-title">{showChat ? 'Workshop' : 'ConusAI'}</span>

				<button
					class="icon-btn"
					aria-label="New conversation"
					onclick={() => { chatStream.newSession(); showChat = false; selectedNodeId = undefined; inputValue = ''; }}
				>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="18" height="18">
						<path d="M9 4v10M4 9h10"/>
					</svg>
				</button>
			</header>

			<main class="main-body">
				{#if showChat}
					<AgentChatStream
						messages={chatStream.messages}
						toolCards={chatStream.toolCards}
						inFlight={chatStream.inFlight}
					/>
					<div class="composer-wrap">
						<AgentChatComposer
							bind:value={inputValue}
							onsubmit={handleSubmit}
							onUpload={handleUpload}
							inFlight={chatStream.inFlight}
						/>
					</div>
				{:else}
					<div class="empty-screen">
						<div class="greeting">
							<div class="greeting-avatar">{initials(user.name)}</div>
							<h2 class="greeting-name">Hi, {user.name.split(' ')[0]}.</h2>
							<p class="greeting-sub">How can I help you today?</p>
						</div>

						<div class="suggestions">
							{#each SUGGESTIONS as s}
								<button class="suggestion-chip" onclick={() => handleSuggestion(s)}>
									{s}
								</button>
							{/each}
						</div>

						<div class="empty-composer">
							<AgentChatComposer
								bind:value={inputValue}
								onsubmit={handleSubmit}
								onUpload={handleUpload}
							/>
						</div>
					</div>
				{/if}
			</main>
		</div>
	</div>
{/if}

<style>
	/* ── Login ──────────────────────────────────────────── */
	.login-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100dvh;
		background: var(--paper);
		padding: calc(var(--s-6) + env(safe-area-inset-top)) var(--s-5) calc(var(--s-6) + env(safe-area-inset-bottom));
		box-sizing: border-box;
	}

	.login-card {
		width: 100%;
		max-width: 380px;
		display: flex;
		flex-direction: column;
		gap: var(--s-6);
	}

	.login-brand {
		display: flex;
		align-items: center;
		gap: var(--s-2);
	}

	.brand-mark {
		width: 38px;
		height: 38px;
		border-radius: 12px;
		background: var(--ember);
		color: #fff;
		font-family: var(--font-display);
		font-size: 21px;
		font-weight: 700;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.brand-name {
		font-family: var(--font-display);
		font-size: var(--t-h3, 18px);
		color: var(--ink);
		font-weight: 600;
	}

	.login-copy { display: flex; flex-direction: column; gap: var(--s-2); }

	.login-title {
		font-family: var(--font-display);
		font-size: clamp(28px, 7vw, 36px);
		color: var(--ink);
		margin: 0;
		line-height: 1.15;
	}

	.login-sub {
		font-size: var(--t-body);
		color: var(--ink-3);
		margin: 0;
		line-height: 1.6;
	}

	.login-form { display: flex; flex-direction: column; gap: var(--s-4); }
	.field { display: flex; flex-direction: column; gap: var(--s-2); }

	.field-label {
		font-size: var(--t-meta);
		font-weight: 600;
		color: var(--ink-2);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.field-input {
		display: block;
		width: 100%;
		box-sizing: border-box;
		padding: 14px var(--s-4);
		border: 1.5px solid var(--rule);
		border-radius: var(--r-md);
		background: var(--paper);
		color: var(--ink);
		font-size: 16px;
		line-height: 1.4;
		transition: border-color 0.15s, box-shadow 0.15s;
		-webkit-appearance: none;
	}

	.field-input:focus {
		outline: none;
		border-color: var(--ember);
		box-shadow: 0 0 0 3px var(--ember-soft);
	}

	.field-input.error { border-color: var(--danger); }

	.field-error {
		font-size: var(--t-meta);
		color: var(--danger);
		margin: 0;
	}

	.plan-fieldset { border: none; padding: 0; margin: 0; }

	.plan-row {
		display: flex;
		gap: var(--s-2);
		margin-top: var(--s-2);
	}

	.plan-option { flex: 1; position: relative; }

	.plan-option input[type="radio"] {
		position: absolute; opacity: 0; width: 0; height: 0;
	}

	.plan-label {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 11px var(--s-2);
		border: 1.5px solid var(--rule);
		border-radius: var(--r-md);
		font-size: var(--t-meta);
		font-weight: 500;
		color: var(--ink-3);
		cursor: pointer;
		text-align: center;
		transition: border-color 0.15s, color 0.15s, background 0.15s;
		-webkit-tap-highlight-color: transparent;
	}

	.plan-option input[type="radio"]:checked + .plan-label {
		border-color: var(--ember);
		color: var(--ember);
		background: var(--ember-soft);
		font-weight: 600;
	}

	.begin-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--s-2);
		width: 100%;
		padding: 15px var(--s-4);
		background: var(--ember);
		color: #fff;
		border: none;
		border-radius: var(--r-md);
		font-size: var(--t-body);
		font-weight: 600;
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
		transition: opacity 0.15s, transform 0.1s;
	}

	.begin-btn:active { opacity: 0.88; transform: scale(0.99); }

	/* ── Workspace shell ────────────────────────────── */
	.workspace {
		display: flex;
		height: 100dvh;
		overflow: hidden;
		position: relative;
		background: var(--paper);
	}

	.sidebar-backdrop {
		display: none;
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.35);
		z-index: 90;
		border: none;
		-webkit-tap-highlight-color: transparent;
	}

	/* ── Sidebar ────────────────────────────────────── */
	.sidebar {
		width: 264px;
		flex-shrink: 0;
		border-right: 1px solid var(--rule);
		background: var(--paper-2);
		display: flex;
		flex-direction: column;
		overflow: hidden;
		transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
		/* Keep user-chip above home indicator on iOS */
		padding-bottom: env(safe-area-inset-bottom);
	}

	/* Sidebar open: takes full workspace, main content hidden */
	.sidebar.open {
		width: 100%;
		flex: 1;
	}

	.hidden { display: none !important; }

	.sidebar-header {
		display: flex;
		align-items: center;
		padding: calc(var(--s-3) + env(safe-area-inset-top)) var(--s-3) var(--s-3) var(--s-4);
		border-bottom: 1px solid var(--rule);
		gap: var(--s-2);
		flex-shrink: 0;
	}

	.sidebar-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		overflow-x: hidden;
	}

	.sidebar-workspace {
		flex-shrink: 0;
		min-height: 120px;
	}

	.sidebar-section {
		border-top: 1px solid var(--rule);
		padding: var(--s-2) 0;
	}

	.section-label {
		padding: var(--s-1) var(--s-4) var(--s-1);
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--ink-3);
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.sidebar-item {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		padding: 6px var(--s-4);
		font-size: var(--t-meta);
		color: var(--ink-2);
		background: none;
		border: none;
		cursor: pointer;
		width: 100%;
		text-align: left;
		-webkit-tap-highlight-color: transparent;
		overflow: hidden;
	}

	.sidebar-item:hover,
	.sidebar-item:active { background: var(--paper-3); }
	.sidebar-item.active { background: var(--ember-soft); color: var(--ink); }

	.sidebar-item-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}

	.sidebar-cap { cursor: default; }
	.sidebar-cap:hover { background: none; }

	.cap-kind {
		font-size: 9px;
		font-family: var(--font-mono);
		color: var(--ember);
		background: var(--ember-soft);
		padding: 1px 5px;
		border-radius: 3px;
		flex-shrink: 0;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.close-btn { display: flex; }

	.avatar {
		width: 34px;
		height: 34px;
		border-radius: 50%;
		background: var(--ember-soft);
		border: 1.5px solid var(--ember-glow);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 13px;
		font-weight: 700;
		color: var(--ember);
		flex-shrink: 0;
	}

	.user-meta {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		gap: 1px;
	}

	.user-name {
		font-size: var(--t-meta);
		font-weight: 600;
		color: var(--ink);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.user-plan {
		font-size: var(--t-label, 11px);
		color: var(--ink-3);
		text-transform: capitalize;
		letter-spacing: 0.02em;
	}

	.logout-btn { flex-shrink: 0; }

	/* ── Icon button (shared) ───────────────────────── */
	.icon-btn {
		width: 44px;
		height: 44px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: none;
		border: none;
		border-radius: var(--r-sm);
		color: var(--ink-3);
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
		flex-shrink: 0;
	}

	.icon-btn:hover { background: var(--paper-3); color: var(--ink); }
	.icon-btn:active { background: var(--paper-3); color: var(--ink); }

	/* ── Main column ────────────────────────────────── */
	.main-col {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}

	/* ── Topbar ─────────────────────────────────────── */
	.topbar {
		display: flex;
		align-items: center;
		/* Push content below status bar/Dynamic Island safe area */
		height: calc(54px + env(safe-area-inset-top));
		padding: env(safe-area-inset-top) var(--s-2) 0;
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
		gap: var(--s-1);
	}

	.topbar-title {
		flex: 1;
		font-family: var(--font-display);
		font-size: var(--t-meta);
		font-weight: 600;
		color: var(--ink-3);
		text-align: center;
		letter-spacing: 0.02em;
	}

	/* ── Main body ──────────────────────────────────── */
	.main-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	/* ── Composer wrap (chat mode) ──────────────────── */
	.composer-wrap {
		flex-shrink: 0;
		padding: var(--s-2) var(--s-3) calc(var(--s-3) + env(safe-area-inset-bottom));
		background: var(--paper);
		border-top: 1px solid var(--rule);
	}

	/* ── Empty / greeting screen ────────────────────── */
	.empty-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: flex-end;
		gap: var(--s-5);
		padding: var(--s-6) var(--s-4) calc(var(--s-4) + env(safe-area-inset-bottom));
		overflow-y: auto;
	}

	.greeting {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--s-3);
		text-align: center;
		margin-top: auto;
		padding-top: var(--s-6);
	}

	.greeting-avatar {
		width: 60px;
		height: 60px;
		border-radius: 50%;
		background: var(--ember-soft);
		border: 2px solid var(--ember-glow);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 22px;
		font-weight: 700;
		color: var(--ember);
	}

	.greeting-name {
		font-family: var(--font-display);
		font-size: clamp(22px, 6vw, 28px);
		color: var(--ink);
		margin: 0;
	}

	.greeting-sub {
		font-size: var(--t-body);
		color: var(--ink-3);
		margin: 0;
	}

	/* ── Suggestion chips ───────────────────────────── */
	.suggestions {
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
		width: 100%;
		max-width: 480px;
	}

	.suggestion-chip {
		display: flex;
		align-items: center;
		text-align: left;
		width: 100%;
		padding: 12px var(--s-4);
		background: var(--paper-2);
		border: 1px solid var(--rule);
		border-radius: var(--r-md);
		font-size: var(--t-meta);
		color: var(--ink-2);
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
		transition: background 0.12s, border-color 0.12s, color 0.12s;
		line-height: 1.4;
	}

	.suggestion-chip:hover,
	.suggestion-chip:active {
		background: var(--paper-3);
		border-color: var(--ember);
		color: var(--ink);
	}

	/* ── Empty screen composer ──────────────────────── */
	.empty-composer {
		width: 100%;
		max-width: 520px;
	}

	/* ── Mobile overrides ───────────────────────────── */
	@media (max-width: 640px) {
		.empty-screen { justify-content: flex-end; }
		.composer-wrap {
			padding: var(--s-2) var(--s-3) calc(var(--s-3) + env(safe-area-inset-bottom));
		}
	}
</style>
