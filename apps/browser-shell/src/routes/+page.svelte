<script lang="ts">
	import { sdk } from '$lib/sdk';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { AgentChatStream, AgentChatComposer, WorkspaceExplorer, createChatStream, type Attachment } from '@conusai/ui/features';
	import { modeStore } from '@conusai/ui/stores';
	import TraceReplayCapability from '$lib/TraceReplayCapability.svelte';
	import type { WorkspaceNode } from '@conusai/types';
	import { onMount } from 'svelte';

	modeStore.setMode('shell');

	const registry = provideCapabilityRendererRegistry();
	registry.register('trace.replay', TraceReplayCapability as any);
	const chatStream = createChatStream(sdk);

	// Workshop session — stored in localStorage, no server round-trip needed.
	let user = $state<{ name: string; plan: string } | null>(null);
	let nameInput = $state('');
	let planInput = $state('enterprise');
	let nameError = $state('');

	let showChat = $state(false);
	let inputValue = $state('');
	let workspaceNodes = $state<WorkspaceNode[]>([]);
	let selectedNodeId = $state<string | undefined>();
	let sidebarOpen = $state(false);

	onMount(() => {
		const raw = localStorage.getItem('conusai_shell_user');
		if (raw) {
			try {
				user = JSON.parse(raw);
				if (user) issueSessionCookie(user.name, user.plan).catch(() => {});
			} catch { /* corrupt — re-login */ }
		}
	});

	async function issueSessionCookie(name: string, plan: string) {
		// Generate an HMAC-SHA256 signed session cookie matching the backend's
		// session.rs format so /ui/* endpoints can authenticate the WKWebView.
		const UI_SESSION_KEY = import.meta.env.VITE_UI_SESSION_KEY ?? 'conusai-foundry-dev-secret-change-me-32b';
		const exp = Math.floor(Date.now() / 1000) + 7 * 86_400; // 7 days
		const payload = JSON.stringify({ name, plan, role: 'user', exp });
		const payloadB64 = btoa(payload).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
		const key = await crypto.subtle.importKey(
			'raw', new TextEncoder().encode(UI_SESSION_KEY),
			{ name: 'HMAC', hash: 'SHA-256' }, false, ['sign'],
		);
		const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(payloadB64));
		const mac = btoa(String.fromCharCode(...new Uint8Array(sig))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
		const apiBase = import.meta.env.VITE_API_BASE ?? '';
		const domain = apiBase ? new URL(apiBase).hostname : 'localhost';
		document.cookie = `conusai_session=${payloadB64}.${mac}; path=/; domain=${domain}; SameSite=None; Secure`;
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
		issueSessionCookie(name, planInput).catch(() => {/* cookie is best-effort */});
	}

	function handleLogout() {
		localStorage.removeItem('conusai_shell_user');
		user = null;
		nameInput = '';
		planInput = 'enterprise';
	}

	function handleSelectNode(node: WorkspaceNode) {
		showChat = true;
		sidebarOpen = false;
		selectedNodeId = node.id;
		if (node.kind === 'conversation' && node.metadata?.thread_id) {
			chatStream.loadThread(node.metadata.thread_id as string);
		}
	}

	function handleSubmit(prompt: string, attachments: Attachment[] = []) {
		if (!prompt.trim()) return;
		showChat = true;
		chatStream.send(prompt, {
			workspaceNodeId: selectedNodeId,
			attachmentIds: attachments.map(a => a.id),
		});
	}

	function initials(name: string) {
		return name.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
	}
</script>

{#if !user}
	<!-- Workshop login — mobile-first, no device token required -->
	<div class="login-screen">
		<div class="login-card" role="main">
			<div class="login-brand">
				<span class="brand-mark">C</span>
				<span class="brand-name">ConusAI</span>
			</div>

			<h1 class="login-title">Enter the workshop.</h1>
			<p class="login-sub">An agent workshop for operators who build with intent.</p>

			<form class="login-form" onsubmit={(e) => { e.preventDefault(); handleBegin(); }}>
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

				<fieldset class="plan-fieldset">
					<legend class="field-label">Plan tier</legend>
					<div class="plan-row">
						<label class="plan-option">
							<input type="radio" name="plan" value="free" bind:group={planInput} />
							<span class="plan-label">Free</span>
						</label>
						<label class="plan-option">
							<input type="radio" name="plan" value="pro" bind:group={planInput} />
							<span class="plan-label">Pro</span>
						</label>
						<label class="plan-option">
							<input type="radio" name="plan" value="enterprise" bind:group={planInput} />
							<span class="plan-label">Enterprise</span>
						</label>
					</div>
				</fieldset>

				<button class="begin-btn" type="submit">
					Begin
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="18" height="18">
						<path d="M5 12h14M12 5l7 7-7 7"/>
					</svg>
				</button>
			</form>
		</div>
	</div>
{:else}
	<!-- Workspace UI — full mobile-first layout -->
	<div class="workspace">
		<!-- Sidebar overlay on mobile -->
		{#if sidebarOpen}
			<button class="sidebar-backdrop" aria-label="Close navigation" onclick={() => sidebarOpen = false}></button>
		{/if}

		<aside class="sidebar" class:open={sidebarOpen} aria-label="Workspace navigation">
			<div class="sidebar-header">
				<span class="sidebar-brand">ConusAI</span>
				<button class="close-btn" aria-label="Close" onclick={() => sidebarOpen = false}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="18" height="18">
						<path d="M4 4l10 10M14 4L4 14"/>
					</svg>
				</button>
			</div>
			<div class="sidebar-body">
				<WorkspaceExplorer {sdk} bind:nodes={workspaceNodes} bind:selectedNodeId onSelectNode={handleSelectNode} />
			</div>
			<div class="user-chip">
				<div class="avatar">{initials(user.name)}</div>
				<div class="user-meta">
					<span class="user-name">{user.name}</span>
					<span class="user-plan">{user.plan}</span>
				</div>
				<button class="logout-btn" aria-label="Sign out" onclick={handleLogout}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="16" height="16">
						<path d="M7 3H3v12h4M12 6l4 3-4 3M6 9h10"/>
					</svg>
				</button>
			</div>
		</aside>

		<div class="main-col">
			<header class="topbar">
				<button class="icon-btn" aria-label="Open navigation" onclick={() => sidebarOpen = !sidebarOpen}>
					<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square" width="18" height="18">
						<line x1="3" y1="5" x2="15" y2="5"/>
						<line x1="3" y1="9" x2="15" y2="9"/>
						<line x1="3" y1="13" x2="15" y2="13"/>
					</svg>
				</button>
				<span class="topbar-title">Workshop</span>
				<button
					class="icon-btn"
					aria-label="New conversation"
					onclick={() => { chatStream.newSession(); showChat = false; selectedNodeId = undefined; }}
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
						<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} inFlight={chatStream.inFlight} />
					</div>
				{:else}
					<div class="empty-screen">
						<div class="greeting">
							<div class="greeting-avatar">{initials(user.name)}</div>
							<h2 class="greeting-text">Good to see you, {user.name.split(' ')[0]}.</h2>
							<p class="greeting-sub">Ask anything to start a conversation.</p>
						</div>
						<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} />
					</div>
				{/if}
			</main>
		</div>
	</div>
{/if}

<style>
	/* ── Login screen ───────────────────────────────── */
	.login-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100dvh;
		background: var(--paper);
		padding: var(--s-6) var(--s-4);
		box-sizing: border-box;
	}

	.login-card {
		width: 100%;
		max-width: 400px;
		display: flex;
		flex-direction: column;
		gap: var(--s-5);
	}

	.login-brand {
		display: flex;
		align-items: center;
		gap: var(--s-2);
	}

	.brand-mark {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		background: var(--ember);
		color: white;
		font-family: var(--font-display);
		font-size: 20px;
		font-weight: 700;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.brand-name {
		font-family: var(--font-display);
		font-size: var(--t-h3);
		color: var(--ink);
	}

	.login-title {
		font-family: var(--font-display);
		font-size: var(--t-h1);
		color: var(--ink);
		margin: 0;
		line-height: 1.2;
	}

	.login-sub {
		font-size: var(--t-body);
		color: var(--ink-3);
		margin: 0;
		line-height: 1.6;
	}

	.login-form {
		display: flex;
		flex-direction: column;
		gap: var(--s-4);
	}

	.field-label {
		display: block;
		font-size: var(--t-label);
		font-weight: 600;
		color: var(--ink-2);
		margin-bottom: var(--s-1);
	}

	.field-input {
		display: block;
		width: 100%;
		box-sizing: border-box;
		padding: 14px var(--s-3);
		border: 1.5px solid var(--rule);
		border-radius: var(--r-sm);
		background: var(--paper);
		color: var(--ink);
		font-size: 16px; /* prevent iOS zoom on focus */
		line-height: 1.4;
		transition: border-color 0.15s;
		-webkit-appearance: none;
		appearance: none;
	}

	.field-input:focus {
		outline: none;
		border-color: var(--ember);
	}

	.field-input.error {
		border-color: var(--rust, #c0392b);
	}

	.field-error {
		font-size: var(--t-label);
		color: var(--rust, #c0392b);
		margin: calc(-1 * var(--s-2)) 0 0;
	}

	.plan-fieldset {
		border: none;
		padding: 0;
		margin: 0;
	}

	.plan-row {
		display: flex;
		gap: var(--s-2);
		margin-top: var(--s-1);
	}

	.plan-option {
		flex: 1;
		position: relative;
	}

	.plan-option input[type="radio"] {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
	}

	.plan-label {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 12px var(--s-2);
		border: 1.5px solid var(--rule);
		border-radius: var(--r-sm);
		font-size: var(--t-body);
		color: var(--ink-3);
		cursor: pointer;
		text-align: center;
		transition: border-color 0.15s, color 0.15s, background 0.15s;
		-webkit-tap-highlight-color: transparent;
	}

	.plan-option input[type="radio"]:checked + .plan-label {
		border-color: var(--ember);
		color: var(--ember);
		background: var(--ember-soft, color-mix(in srgb, var(--ember) 10%, transparent));
		font-weight: 600;
	}

	.begin-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--s-2);
		width: 100%;
		padding: 16px var(--s-4);
		background: var(--ember);
		color: white;
		border: none;
		border-radius: var(--r-sm);
		font-size: var(--t-body);
		font-weight: 600;
		cursor: pointer;
		-webkit-tap-highlight-color: transparent;
		transition: opacity 0.15s;
		margin-top: var(--s-2);
	}

	.begin-btn:active { opacity: 0.8; }

	/* ── Workspace ──────────────────────────────────── */
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
		background: rgba(0, 0, 0, 0.4);
		z-index: 90;
		border: none;
		-webkit-tap-highlight-color: transparent;
	}

	.sidebar {
		width: 260px;
		flex-shrink: 0;
		border-right: 1px solid var(--rule);
		background: var(--paper-2);
		display: flex;
		flex-direction: column;
		overflow: hidden;
		transition: transform 0.25s ease;
	}

	.sidebar-header {
		display: flex;
		align-items: center;
		padding: var(--s-3) var(--s-4);
		border-bottom: 1px solid var(--rule);
		gap: var(--s-2);
	}

	.sidebar-brand {
		flex: 1;
		font-family: var(--font-display);
		font-size: var(--t-h3);
		color: var(--ember);
	}

	.sidebar-body {
		flex: 1;
		overflow: hidden;
	}

	.close-btn {
		display: none;
		width: 36px;
		height: 36px;
		align-items: center;
		justify-content: center;
		background: none;
		border: none;
		border-radius: var(--r-sm);
		color: var(--ink-3);
		cursor: pointer;
	}

	.user-chip {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		padding: var(--s-3) var(--s-4);
		padding-bottom: max(var(--s-3), env(safe-area-inset-bottom));
		border-top: 1px solid var(--rule);
	}

	.avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--ember-soft, color-mix(in srgb, var(--ember) 15%, transparent));
		border: 1px solid var(--ember-glow, color-mix(in srgb, var(--ember) 30%, transparent));
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: var(--t-label);
		font-weight: 700;
		color: var(--ember);
		flex-shrink: 0;
	}

	.user-meta {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.user-name {
		font-size: var(--t-body);
		color: var(--ink);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.user-plan {
		font-size: var(--t-label);
		color: var(--ink-3);
		text-transform: capitalize;
	}

	.logout-btn {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: none;
		border: none;
		border-radius: var(--r-sm);
		color: var(--ink-3);
		cursor: pointer;
		flex-shrink: 0;
	}

	.logout-btn:hover, .close-btn:hover { background: var(--paper-3); color: var(--ink); }

	.main-col {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 0;
	}

	.topbar {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		height: 52px;
		padding: 0 var(--s-3);
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
	}

	.topbar-title {
		flex: 1;
		font-family: var(--font-display);
		font-size: var(--t-body);
		color: var(--ink-3);
		text-align: center;
	}

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
	}

	.icon-btn:active { background: var(--paper-3); color: var(--ink); }

	.main-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.composer-wrap {
		flex-shrink: 0;
		padding: var(--s-3) var(--s-3) calc(var(--s-4) + env(safe-area-inset-bottom));
	}

	.empty-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--s-6);
		padding: var(--s-8) var(--s-4) calc(var(--s-8) + env(safe-area-inset-bottom));
	}

	.greeting {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--s-3);
		text-align: center;
	}

	.greeting-avatar {
		width: 56px;
		height: 56px;
		border-radius: 50%;
		background: var(--ember-soft, color-mix(in srgb, var(--ember) 15%, transparent));
		border: 2px solid var(--ember-glow, color-mix(in srgb, var(--ember) 30%, transparent));
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 22px;
		font-weight: 700;
		color: var(--ember);
	}

	.greeting-text {
		font-family: var(--font-display);
		font-size: var(--t-h2);
		color: var(--ink);
		margin: 0;
	}

	.greeting-sub {
		font-size: var(--t-body);
		color: var(--ink-3);
		margin: 0;
	}

	/* ── Mobile overrides (≤640px) ──────────────────── */
	@media (max-width: 640px) {
		.sidebar {
			position: fixed;
			inset: 0 auto 0 0;
			z-index: 100;
			transform: translateX(-100%);
			width: min(80vw, 300px);
			/* Prevent user-chip from hiding behind the home indicator */
			padding-bottom: env(safe-area-inset-bottom);
		}

		.sidebar.open {
			transform: translateX(0);
			box-shadow: 4px 0 32px rgba(0, 0, 0, 0.2);
		}

		.sidebar-backdrop { display: block; }
		.close-btn { display: flex; }

		.topbar { height: 52px; }

		.composer-wrap {
			padding: var(--s-2) var(--s-3) calc(var(--s-3) + env(safe-area-inset-bottom));
		}
	}
</style>
