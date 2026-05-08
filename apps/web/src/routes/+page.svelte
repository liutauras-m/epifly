<script lang="ts">
	import type { PageData } from './$types';
	import { onMount } from 'svelte';

	let { data }: { data: PageData } = $props();

	// ── State ────────────────────────────────────────────────────────────────
	let showChat = $state(false);
	let messages: { role: 'user' | 'ai' | 'thinking'; text: string; streaming?: boolean }[] = $state([]);
	let toolCards: Map<string, { name: string; status: 'running' | 'success' | 'error'; result: string; startTime: number }> = $state(new Map());
	let activeThreadId = $state<string | null>(null);
	let inFlight = $state(false);
	let inputValue = $state('');
	let pendingAttachments: { id: string; filename: string; size: number }[] = $state([]);

	// ── Theme ────────────────────────────────────────────────────────────────
	let theme = $state('paper');
	onMount(() => {
		theme = localStorage.getItem('conusai-theme') ?? 'paper';
		document.documentElement.setAttribute('data-theme', theme);
	});
	function toggleTheme() {
		theme = theme === 'paper' ? 'forge' : 'paper';
		document.documentElement.setAttribute('data-theme', theme);
		localStorage.setItem('conusai-theme', theme);
	}

	// ── Chat ─────────────────────────────────────────────────────────────────
	let messagesEl: HTMLDivElement;
	function scrollIfNear() {
		if (!messagesEl) return;
		const { scrollHeight, scrollTop, clientHeight } = messagesEl;
		if (scrollHeight - scrollTop - clientHeight < 120) messagesEl.scrollTop = scrollHeight;
	}

	async function streamChat(prompt: string) {
		if (inFlight || !prompt.trim()) return;
		inFlight = true;
		showChat = true;
		messages = [...messages, { role: 'user', text: prompt }];
		messages = [...messages, { role: 'thinking', text: '' }];
		await tick();
		scrollIfNear();

		let aiIdx = -1;
		const newToolCards = new Map(toolCards);

		try {
			const body: Record<string, unknown> = { message: prompt, thread_id: activeThreadId };
			const res = await fetch('/ui/stream', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(body)
			});
			if (!res.ok || !res.body) {
				messages = messages.filter((m) => m.role !== 'thinking');
				messages = [...messages, { role: 'ai', text: `Error: ${res.status} ${res.statusText}` }];
				return;
			}
			messages = messages.filter((m) => m.role !== 'thinking');

			const reader = res.body.getReader();
			const dec = new TextDecoder();
			let buf = '';

			while (true) {
				const { value, done } = await reader.read();
				if (done) break;
				buf += dec.decode(value, { stream: true });
				let pos: number;
				while ((pos = buf.indexOf('\n\n')) !== -1) {
					const block = buf.slice(0, pos);
					buf = buf.slice(pos + 2);
					for (const line of block.split('\n')) {
						if (!line.startsWith('data: ')) continue;
						const raw = line.slice(6);
						if (raw === '[DONE]') continue;
						let ev: Record<string, unknown>;
						try { ev = JSON.parse(raw); } catch { continue; }
						const delta = (ev.choices as { delta?: Record<string, unknown> }[])?.[0]?.delta;
						if (!delta) continue;

						if (typeof delta.content === 'string') {
							if (aiIdx < 0 || messages[aiIdx]?.role !== 'ai') {
								messages = [...messages, { role: 'ai', text: '', streaming: true }];
								aiIdx = messages.length - 1;
							}
							messages[aiIdx] = { ...messages[aiIdx], text: messages[aiIdx].text + delta.content };
							messages = [...messages];
							scrollIfNear();
						} else if (delta.tool_call_start) {
							const { id, name } = delta.tool_call_start as { id: string; name: string };
							newToolCards.set(id, { name, status: 'running', result: '', startTime: performance.now() });
							toolCards = new Map(newToolCards);
							aiIdx = -1;
						} else if (delta.tool_call_result) {
							const { tool_use_id, result } = delta.tool_call_result as { tool_use_id: string; result: string };
							const card = newToolCards.get(tool_use_id);
							if (card) {
								let isError = false;
								try { const obj = JSON.parse(result); if (obj?.error || obj?.status === 'error') isError = true; } catch {}
								if (typeof result === 'string' && result.startsWith('Error:')) isError = true;
								newToolCards.set(tool_use_id, { ...card, status: isError ? 'error' : 'success', result });
								toolCards = new Map(newToolCards);
							}
						}

						if (ev.thread_id) activeThreadId = ev.thread_id as string;
					}
				}
			}
			if (aiIdx >= 0) messages[aiIdx] = { ...messages[aiIdx], streaming: false };
			messages = [...messages];
		} catch (e: unknown) {
			messages = messages.filter((m) => m.role !== 'thinking');
			messages = [...messages, { role: 'ai', text: `Stream failed: ${e instanceof Error ? e.message : String(e)}` }];
		} finally {
			inFlight = false;
		}
	}

	// ── Invoice extraction ────────────────────────────────────────────────────
	let invoiceResults: Map<string, unknown> = $state(new Map());
	async function extractInvoice(token: string, filename: string) {
		if (inFlight) return;
		inFlight = true;
		showChat = true;
		messages = [...messages, { role: 'user', text: `Extract invoice data from ${filename}` }];
		messages = [...messages, { role: 'ai', text: 'Running invoice pipeline…', streaming: true }];
		const loadIdx = messages.length - 1;
		try {
			const res = await fetch('/ui/extract-invoice', {
				method: 'POST', headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ token })
			});
			messages = messages.filter((_, i) => i !== loadIdx);
			if (!res.ok) { messages = [...messages, { role: 'ai', text: `Extraction failed: ${res.statusText}` }]; return; }
			const d = await res.json();
			invoiceResults = new Map([...invoiceResults, [token, d]]);
			messages = [...messages, { role: 'ai', text: '__invoice__' + token }];
		} catch (e: unknown) {
			messages = messages.filter((_, i) => i !== loadIdx);
			messages = [...messages, { role: 'ai', text: `Error: ${e instanceof Error ? e.message : String(e)}` }];
		} finally { inFlight = false; }
	}

	// ── Upload ────────────────────────────────────────────────────────────────
	async function uploadFiles(files: File[]) {
		for (const file of files) {
			const fd = new FormData();
			fd.append('file', file, file.name);
			const res = await fetch('/ui/upload', { method: 'POST', body: fd });
			if (!res.ok) continue;
			const d = await res.json() as { id: string; filename: string; size: number };
			pendingAttachments = [...pendingAttachments, { id: d.id, filename: d.filename, size: d.size }];
		}
	}

	function fmtSize(n: number) {
		if (n < 1024) return `${n}B`;
		if (n < 1048576) return `${(n / 1024).toFixed(1)}KB`;
		return `${(n / 1048576).toFixed(1)}MB`;
	}
	const INVOICE_EXT = /\.(png|jpg|jpeg|pdf)$/i;
	const INVOICE_NAME = /invoice|receipt|bill|facture/i;
	const isInvoice = (a: { filename: string }) => INVOICE_EXT.test(a.filename) && INVOICE_NAME.test(a.filename);

	// ── Submit composer ────────────────────────────────────────────────────────
	function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		const val = inputValue.trim();
		if (!val && pendingAttachments.length === 0) return;
		let prompt = val;
		if (pendingAttachments.length) {
			const origin = window.location.origin;
			const lines = pendingAttachments.map((a) => `- ${a.filename} (image_path: ${origin}/v1/files/${a.id})`).join('\n');
			prompt = `${val}\n\n[Attached files]\n${lines}`;
		}
		inputValue = '';
		pendingAttachments = [];
		streamChat(prompt);
	}

	// ── Textarea auto-grow ─────────────────────────────────────────────────────
	function grow(el: HTMLTextAreaElement) {
		el.style.height = 'auto';
		el.style.height = Math.min(el.scrollHeight, 240) + 'px';
	}

	// ── Load thread history ───────────────────────────────────────────────────
	async function loadThread(threadId: string) {
		if (inFlight) return;
		showChat = true;
		activeThreadId = threadId;
		messages = [{ role: 'ai', text: 'Loading…', streaming: true }];
		try {
			const res = await fetch(`/v1/threads/${threadId}/messages`, {
				headers: { 'X-Tenant-ID': 'dev' }
			});
			if (!res.ok) { messages = [{ role: 'ai', text: 'Could not load thread.' }]; return; }
			const raw = await res.json() as unknown;
			const arr = Array.isArray(raw) ? raw : (raw as { messages?: unknown[] }).messages ?? [];
			messages = (arr as { role: string; content: string }[]).map((m) => ({
				role: m.role === 'user' ? 'user' : 'ai',
				text: m.content
			}));
		} catch { messages = [{ role: 'ai', text: 'Failed to load thread.' }]; }
	}

	function newChat() {
		showChat = false;
		messages = [];
		activeThreadId = null;
		toolCards = new Map();
	}

	// ── Keyboard shortcuts ──────────────────────────────────────────────────
	function onKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'n') { e.preventDefault(); newChat(); }
		if (mod && e.key === '/') { e.preventDefault(); toggleTheme(); }
	}

	// ── Svelte tick ────────────────────────────────────────────────────────
	import { tick } from 'svelte';

	// ── Mobile sidebar ─────────────────────────────────────────────────────
	let sidebarOpen = $state(false);

	// ── Drag & drop on composer ─────────────────────────────────────────────
	let dropTarget = $state(false);
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head>
	<title>Workshop · ConusAI</title>
	<script src="/js/workspace.js" type="module"></script>
</svelte:head>

<div class="app">
	<!-- ── Sidebar ── -->
	<aside class="sidebar" class:open={sidebarOpen} aria-label="Workshop navigation">
		<section class="nav-section ws-section" aria-labelledby="ws-heading">
			<header class="nav-header">
				<span id="ws-heading" class="nav-heading label-mono">Workspace</span>
				<button type="button" class="icon-btn ws-new-btn" aria-label="New folder or conversation">
					<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
						<line x1="9" y1="3" x2="9" y2="15"/><line x1="3" y1="9" x2="15" y2="9"/>
					</svg>
				</button>
			</header>
			<div class="ws-search-wrap">
				<svg class="ws-search-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
					<circle cx="6.5" cy="6.5" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/>
				</svg>
				<input id="ws-search" class="ws-search-input" type="search" placeholder="Search conversations…" autocomplete="off" spellcheck="false" aria-label="Search workspace">
				<button class="ws-search-clear" id="ws-search-clear" aria-label="Clear search" hidden>
					<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
						<line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
					</svg>
				</button>
			</div>
			<div id="workspace-tree" class="ws-tree" role="tree" aria-busy="true" aria-labelledby="ws-heading">
				<div class="ws-skeleton" aria-hidden="true"></div>
			</div>
		</section>

		<div class="nav-section">
			<div class="nav-heading label-mono">Recents</div>
			<div class="recents-list" id="recents-list">
				{#each data.recents as r (r.id)}
					<div class="recent" role="button" tabindex="0"
						onclick={() => loadThread(r.id)}
						onkeydown={(e) => e.key === 'Enter' && loadThread(r.id)}
						data-thread-id={r.id}>{r.title}</div>
				{:else}
					<div class="empty-hint">No threads yet — start a new chat to forge one.</div>
				{/each}
			</div>
		</div>

		<div class="nav-section">
			<div class="nav-heading label-mono">Capabilities</div>
			<div class="cap-list">
				{#each data.capabilities as c (c.name)}
					<div class="cap" role="button" tabindex="0"
						onclick={() => { inputValue = (inputValue ? inputValue + ' ' : '') + '@' + c.name + ' '; }}
						onkeydown={(e) => e.key === 'Enter' && (inputValue = '@' + c.name + ' ')}>
						<span class="cap-glyph">{c.kindGlyph}</span>
						<span class="cap-name">{c.name}</span>
						<span class="cap-count">{c.toolCount}</span>
					</div>
				{:else}
					<div class="empty-hint">No capabilities loaded.</div>
				{/each}
			</div>
		</div>

		<div class="user-chip">
			<div class="avatar">{data.user?.initials ?? '?'}</div>
			<div class="user-meta">
				<span class="user-name">{data.user?.name ?? ''}</span>
				<span class="user-plan">{data.user?.plan ?? ''}</span>
			</div>
		</div>
	</aside>

	<!-- ── Main ── -->
	<main class="main">
		<div class="topbar">
			<button class="icon-btn menu-btn" aria-label="Toggle navigation"
				onclick={() => (sidebarOpen = !sidebarOpen)}>
				<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square">
					<line x1="3" y1="5" x2="15" y2="5"/><line x1="3" y1="9" x2="15" y2="9"/><line x1="3" y1="13" x2="15" y2="13"/>
				</svg>
			</button>
			<div style="flex:1"></div>
			<button class="icon-btn" aria-label="New chat" onclick={newChat} title="New chat (⌘N)">
				<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
					<line x1="9" y1="3" x2="9" y2="15"/><line x1="3" y1="9" x2="15" y2="9"/>
				</svg>
			</button>
			<button class="icon-btn" id="theme-toggle" aria-label="Toggle theme" onclick={toggleTheme}>
				<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5">
					{#if theme === 'forge'}
						<circle cx="9" cy="9" r="4"/><line x1="9" y1="1" x2="9" y2="3"/><line x1="9" y1="15" x2="9" y2="17"/><line x1="1" y1="9" x2="3" y2="9"/><line x1="15" y1="9" x2="17" y2="9"/>
					{:else}
						<path d="M14 10a6 6 0 0 1-8-8 7 7 0 1 0 8 8z"/>
					{/if}
				</svg>
			</button>
			<a href="/logout" class="icon-btn" aria-label="Logout">
				<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
					<path d="M7 3H3v12h4M12 6l4 3-4 3M6 9h10"/>
				</svg>
			</a>
		</div>
		{#if sidebarOpen}
			<div class="sidebar-backdrop" role="button" tabindex="-1"
				onclick={() => (sidebarOpen = false)} onkeydown={() => {}}></div>
		{/if}

		<!-- ── Greeting screen ── -->
		{#if !showChat}
			<section class="greeting-screen">
				<div class="greeting">
					<img class="sigil" src="/images/favicon.png" alt="" aria-hidden="true">
					<h1 class="greeting-text">Good {data.user ? '' : ''}morning, {data.user?.firstName ?? 'there'}</h1>
				</div>

				{@render composer()}

				<div class="chips">
					{#each [['Code','Help me write code that '],['Write','Help me write '],['Learn','Teach me about '],['Life stuff','Help me with '],["Operator's choice",'Pick the best approach for ']] as [label, prompt]}
						<button class="chip" onclick={() => { inputValue = prompt; }}>
							{label}
						</button>
					{/each}
				</div>
			</section>
		{/if}

		<!-- ── Chat view ── -->
		{#if showChat}
			<section class="chat-view">
				<div class="messages" bind:this={messagesEl} role="log" aria-live="polite">
					{#each messages as msg, i (i)}
						{#if msg.role === 'thinking'}
							<div class="message ai thinking">
								<span class="thinking-dots" aria-label="Thinking"><i></i><i></i><i></i></span>
							</div>
						{:else if msg.role === 'user'}
							<div class="message user">{msg.text}</div>
						{:else if msg.text.startsWith('__invoice__')}
							{@const token = msg.text.slice('__invoice__'.length)}
							{@const inv = invoiceResults.get(token)}
							{#if inv}
								{@render invoiceCard(inv as InvoiceData, token)}
							{/if}
						{:else}
							<div class="message ai" class:streaming={msg.streaming}>
								<span class="ai-text">{msg.text}</span>
								{#if msg.streaming}<span class="cursor" aria-hidden="true"></span>{/if}
							</div>
						{/if}
					{/each}

					{#each [...toolCards.entries()] as [id, card] (id)}
						<details class="tool-card" data-status={card.status}>
							<summary class="tool-head">
								<span class="tool-dot" role="status" aria-label={card.status}></span>
								<span class="tool-name">{card.name}</span>
								<span class="tool-time">
									{#if card.status !== 'running'}
										{Math.round(performance.now() - card.startTime)}ms
									{:else}…{/if}
								</span>
							</summary>
							<div class="tool-body">{card.result || 'running…'}</div>
						</details>
					{/each}
				</div>

				<div class="composer-bottom">
					{@render composer()}
				</div>
			</section>
		{/if}
	</main>
</div>

<!-- ── Composer snippet ────────────────────────────────────────────────────── -->
{#snippet composer()}
	<div class="composer-wrap">
		<form class="composer" class:drop-target={dropTarget}
			onsubmit={handleSubmit}
			ondragover={(e) => { if (e.dataTransfer?.types?.includes('Files')) { e.preventDefault(); dropTarget = true; } }}
			ondragleave={() => (dropTarget = false)}
			ondrop={(e) => { e.preventDefault(); dropTarget = false; if (e.dataTransfer?.files?.length) uploadFiles([...e.dataTransfer.files]); }}>

			{#if pendingAttachments.length}
				<div class="attachments">
					{#each pendingAttachments as a (a.id)}
						<span class="attachment">
							<span class="attachment-thumb">
								<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 2h7l4 4v11H4z"/><polyline points="11,2 11,6 15,6"/></svg>
							</span>
							<span class="attachment-name">{a.filename}</span>
							<span class="attachment-size">{fmtSize(a.size)}</span>
							{#if isInvoice(a)}
								<button type="button" class="attachment-extract"
									onclick={() => { pendingAttachments = pendingAttachments.filter(x => x.id !== a.id); extractInvoice(a.id, a.filename); }}>
									Extract invoice
								</button>
							{/if}
							<button type="button" class="attachment-remove" aria-label="Remove"
								onclick={() => (pendingAttachments = pendingAttachments.filter(x => x.id !== a.id))}>
								<svg class="icon" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
									<line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
								</svg>
							</button>
						</span>
					{/each}
				</div>
			{/if}

			<label class="sr-only" for="prompt">Message</label>
			<textarea id="prompt" class="composer-input" name="prompt" placeholder="How can I help you today?"
				rows="2" autocomplete="off" bind:value={inputValue}
				oninput={(e) => grow(e.currentTarget)}
				onkeydown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); (e.currentTarget.closest('form') as HTMLFormElement)?.requestSubmit(); } }}></textarea>

			<div class="composer-toolbar">
				<button type="button" class="toolbar-btn" aria-label="Attach file"
					onclick={() => document.getElementById('file-input')?.click()}>
					<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
						<path d="M15 9l-6 6a4 4 0 0 1-5.657-5.657l7-7a2.5 2.5 0 0 1 3.536 3.536l-7 7a1 1 0 0 1-1.414-1.414l6-6"/>
					</svg>
				</button>
				<input id="file-input" type="file" hidden multiple
					onchange={(e) => { const files = e.currentTarget.files; if (files?.length) uploadFiles([...files]); e.currentTarget.value = ''; }}>
				<div class="toolbar-spacer"></div>
				<span class="model-pill">Opus 4.7</span>
				<button type="submit" class="send-btn" aria-label="Send" disabled={inFlight}>
					<svg class="icon" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round">
						<line x1="7" y1="12" x2="7" y2="2"/><polyline points="3,6 7,2 11,6"/>
					</svg>
				</button>
			</div>
		</form>
	</div>
{/snippet}

<!-- ── Invoice card snippet ────────────────────────────────────────────────── -->
{#snippet invoiceCard(inv: InvoiceData, filename: string)}
	{@const cur = inv.currency ?? ''}
	{@const fmt = (v: unknown) => v == null ? '—' : String(v)}
	{@const fmtM = (v: unknown) => v == null ? '—' : `${cur}${Number(v).toFixed(2)}`}
	<div class="message ai invoice-result">
		<div class="inv-card">
			<div class="inv-header">
				<div class="inv-title-row">
					<span class="inv-label">Invoice</span>
					<strong class="inv-number">{fmt(inv.invoice_number)}</strong>
					{#if inv.status}<span class="inv-badge inv-badge-{inv.status.toLowerCase()}">{inv.status}</span>{/if}
				</div>
				<div class="inv-meta">
					{#if inv.invoice_date}<span>Date: <b>{inv.invoice_date}</b></span>{/if}
					{#if inv.due_date}<span>Due: <b>{inv.due_date}</b></span>{/if}
				</div>
			</div>
			<div class="inv-parties">
				<div class="inv-party">
					<div class="inv-party-label">From</div>
					<div class="inv-party-name">{fmt(inv.issuer_name)}</div>
					{#if inv.issuer_address}<div class="inv-party-detail">{inv.issuer_address}</div>{/if}
				</div>
				<div class="inv-party">
					<div class="inv-party-label">To</div>
					<div class="inv-party-name">{fmt(inv.billed_to_name)}</div>
					{#if inv.billed_to_company}<div class="inv-party-detail">{inv.billed_to_company}</div>{/if}
				</div>
			</div>
			{#if inv.line_items?.length}
				<table class="inv-table">
					<thead><tr><th>Description</th><th>Qty</th><th>Unit Price</th><th>Total</th></tr></thead>
					<tbody>
						{#each inv.line_items as li}
							<tr>
								<td>{li.description ?? ''}</td>
								<td class="inv-num">{fmt(li.quantity)}</td>
								<td class="inv-num">{fmtM(li.unit_price)}</td>
								<td class="inv-num">{fmtM(li.total)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
			<div class="inv-totals">
				{#if inv.subtotal != null}<div class="inv-total-row"><span>Subtotal</span><span>{fmtM(inv.subtotal)}</span></div>{/if}
				{#if inv.tax_amount != null}<div class="inv-total-row"><span>Tax</span><span>{fmtM(inv.tax_amount)}</span></div>{/if}
				<div class="inv-total-row inv-grand-total"><span>Total</span><span>{fmtM(inv.total_amount)}</span></div>
			</div>
			<div class="inv-source">Extracted from {filename} via InvoicePipeline</div>
		</div>
	</div>
{/snippet}

<script lang="ts" context="module">
	interface InvoiceData {
		invoice_number?: unknown; status?: string; invoice_date?: string; due_date?: string;
		issuer_name?: unknown; issuer_address?: string;
		billed_to_name?: unknown; billed_to_company?: string;
		currency?: string; subtotal?: number; tax_amount?: number; total_amount?: number;
		line_items?: { description?: string; quantity?: unknown; unit_price?: unknown; total?: unknown }[];
	}
</script>
