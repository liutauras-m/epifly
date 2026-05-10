<script lang="ts">
	import type { PageData } from './$types';
	import type { WorkspaceNode } from '$lib/types';
	import type { InvoiceData } from '$lib/api/types';
	import { streamChat as apiStreamChat } from '$lib/api/stream';
	import { workspacesApi, apiCall } from '$lib/api';
	import { EP } from '$lib/api/endpoints';
	import { toasts } from '$lib/ui/toast.svelte';
	import { autoGrow } from '$lib/ui/actions';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';

	let { data }: { data: PageData } = $props();

	// ── State ────────────────────────────────────────────────────────────────
	let showChat = $state(false);
	// words: word tokens for animated streaming display; cleared when streaming ends
	let messages: { role: 'user' | 'ai' | 'thinking'; text: string; streaming?: boolean; words?: { t: string; id: number; delay: number }[] }[] = $state([]);
	let toolCards: Map<string, { name: string; status: 'running' | 'success' | 'error'; result: string; startTime: number }> = $state(new Map());
	let activeThreadId = $state<string | null>(null);
	let inFlight = $state(false);
	let streamController = $state<AbortController | null>(null);
	let inputValue = $state('');
	let pendingAttachments: { id: string; filename: string; size: number }[] = $state([]);
	let composerFocused = $state(false);

	// ── Workspace tree ────────────────────────────────────────────────────────
	let workspaceNodes = $state<WorkspaceNode[]>(data.workspaceTree ?? []);
	let expandedFolders = $state<Set<string>>(new Set());
	let childNodes = $state<Map<string, WorkspaceNode[]>>(new Map());
	let selectedNodeId = $state<string | null>(null);

	onMount(() => {
		const wsParam = $page.url.searchParams.get('ws');
		if (wsParam) selectedNodeId = wsParam;
		try {
			const stored = localStorage.getItem('conusai-recents');
			if (stored) recents = JSON.parse(stored);
		} catch { /* ignore */ }
	});

	async function toggleFolder(node: WorkspaceNode) {
		if (expandedFolders.has(node.id)) {
			expandedFolders.delete(node.id);
			expandedFolders = new Set(expandedFolders);
		} else {
			expandedFolders.add(node.id);
			expandedFolders = new Set(expandedFolders);
			if (!childNodes.has(node.id)) {
				try {
					const result = await workspacesApi.getTree(fetch, node.id);
					if (!result.error) {
						const raw = result.data;
						const nodes: WorkspaceNode[] = Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []);
						const updated = new Map(childNodes);
						updated.set(node.id, nodes);
						childNodes = updated;
					}
				} catch { /* ignore */ }
			}
		}
	}

	async function selectNode(node: WorkspaceNode) {
		selectedNodeId = node.id;
		goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true });
		if (node.kind !== 'conversation') return;

		showChat = true;
		messages = [];

		// If the node already has a bound thread, load its history
		const tid = node.metadata?.thread_id ?? null;
		if (tid) {
			activeThreadId = tid;
			messages = [{ role: 'ai', text: '…', streaming: true }];
			try {
				type MsgArr = { role: string; content: unknown }[];
				const result = await apiCall<MsgArr | { data?: MsgArr }>(fetch, EP.THREAD_MESSAGES(tid));
				if (!result.error) {
					const raw = result.data;
					const msgs: MsgArr = Array.isArray(raw) ? raw : ((raw as { data?: MsgArr })?.data ?? []);
					messages = msgs
						.filter((m) => m.role === 'user' || m.role === 'assistant')
						.map((m) => ({
							role: (m.role === 'assistant' ? 'ai' : 'user') as 'ai' | 'user',
							text: typeof m.content === 'string' ? m.content
								: Array.isArray(m.content)
									? (m.content as { type: string; text?: string }[])
											.filter((b) => b.type === 'text')
											.map((b) => b.text ?? '')
											.join('')
									: String(m.content),
							streaming: false
						}));
					if (messages.length === 0) {
						messages = [{ role: 'ai', text: `_${node.virtual_path} — no messages yet._`, streaming: false }];
					}
				} else {
					messages = [{ role: 'ai', text: `_${node.virtual_path}_`, streaming: false }];
				}
			} catch {
				messages = [{ role: 'ai', text: `_${node.virtual_path}_`, streaming: false }];
			}
		} else {
			// No thread yet — show the file name, chat will bind one on first message
			activeThreadId = null;
			messages = [{ role: 'ai', text: `_${node.virtual_path} — send a message to start._`, streaming: false }];
		}
	}

	/** After a stream bound to a workspace node completes, re-fetch that node
	 *  so its metadata.thread_id is populated for the next selectNode call. */
	async function refreshNodeMetadata(nodeId: string) {
		try {
			const result = await workspacesApi.getNode(fetch, nodeId);
			if (result.error) return;
			const updated: WorkspaceNode = result.data;
			// Update the node in all trees in-place
			function patchIn(nodes: WorkspaceNode[]): boolean {
				for (let i = 0; i < nodes.length; i++) {
					if (nodes[i].id === nodeId) { nodes[i] = updated; return true; }
				}
				return false;
			}
			if (!patchIn(workspaceNodes)) {
				for (const children of childNodes.values()) patchIn(children);
			}
			workspaceNodes = [...workspaceNodes];
		} catch { /* ignore */ }
	}

	async function refreshWorkspaceTree() {
		try {
			const result = await workspacesApi.getTree(fetch);
			if (!result.error) {
				const raw = result.data;
				workspaceNodes = Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []);
			}
		} catch { /* ignore */ }
	}

	// ── Workspace creation ────────────────────────────────────────────────────
	let newNodeKind = $state<'folder' | 'conversation'>('folder');
	let newNodeName = $state('');
	let newNodeParentId = $state<string | null>(null);
	let showNewNodeForm = $state(false);
	let newNodeError = $state('');
	let newNodeBusy = $state(false);

	function selectedFolderParent(): string | null {
		if (!selectedNodeId) return null;
		const allNodes = [...workspaceNodes, ...[...childNodes.values()].flat()];
		const node = allNodes.find((n) => n.id === selectedNodeId);
		return node?.kind === 'folder' ? selectedNodeId : null;
	}

	function openNewNodeForm(parentId: string | null = null) {
		newNodeParentId = parentId;
		newNodeName = '';
		newNodeError = '';
		newNodeKind = 'folder';
		showNewNodeForm = true;
	}

	function closeNewNodeForm() {
		showNewNodeForm = false;
		newNodeName = '';
		newNodeError = '';
	}

	async function submitNewNode(e: SubmitEvent) {
		e.preventDefault();
		let name = newNodeName.trim();
		if (!name) { newNodeError = 'Name is required'; return; }
		if (newNodeKind === 'conversation' && !name.endsWith('.md')) name = `${name}.md`;
		newNodeBusy = true;
		newNodeError = '';
		try {
			const result = await workspacesApi.createNode(fetch, {
				kind: newNodeKind,
				name,
				parent_id: newNodeParentId
			});
			if (result.error) {
				newNodeError = result.error.message ?? `Error ${result.error.status}`;
				return;
			}
			const createdNode = result.data;
			closeNewNodeForm();
			// If created inside a folder, expand it and refresh its children
			if (newNodeParentId) {
				expandedFolders.add(newNodeParentId);
				expandedFolders = new Set(expandedFolders);
				try {
					const childResult = await workspacesApi.getTree(fetch, newNodeParentId);
					if (!childResult.error) {
						const raw = childResult.data;
						const nodes: WorkspaceNode[] = Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []);
						const updated = new Map(childNodes);
						updated.set(newNodeParentId, nodes);
						childNodes = updated;
					}
				} catch { /* ignore */ }
				// Select the newly created node if it's a conversation
				if (createdNode && newNodeKind === 'conversation') {
					selectedNodeId = createdNode.id;
					history.replaceState(null, '', `?ws=${createdNode.id}`);
				}
			} else {
				await refreshWorkspaceTree();
			}
		} catch (err) {
			newNodeError = err instanceof Error ? err.message : 'Network error';
		} finally {
			newNodeBusy = false;
		}
	}

	function focusInput(el: HTMLInputElement) {
		el.focus();
	}

	// ── Workspace search (debounced) ─────────────────────────────────────────
	let searchQuery = $state('');
	let searchResults = $state<WorkspaceNode[]>([]);
	let searchTimer: ReturnType<typeof setTimeout> | null = null;

	function onSearchInput(e: Event) {
		const q = (e.target as HTMLInputElement).value;
		searchQuery = q;
		if (searchTimer) clearTimeout(searchTimer);
		if (!q.trim()) { searchResults = []; return; }
		searchTimer = setTimeout(async () => {
			try {
				const result = await workspacesApi.searchNodes(fetch, q.trim());
				if (!result.error) {
					const raw = result.data;
					searchResults = Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []);
				}
			} catch { searchResults = []; }
		}, 220);
	}

	function clearSearch() {
		searchQuery = '';
		searchResults = [];
		if (searchTimer) clearTimeout(searchTimer);
	}

	// ── Recents (local, updated after chat) ──────────────────────────────────
	let recents = $state<{ id: string; title: string }[]>([]);

	// ── Theme ────────────────────────────────────────────────────────────────
	let theme = $state('paper');
	onMount(() => {
		// Priority: flash-prevention script set data-theme → localStorage → system preference
		const stored = localStorage.getItem('conusai-theme');
		const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
		theme = document.documentElement.dataset.theme
			?? stored
			?? (systemDark ? 'forge' : 'paper');
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

		const controller = new AbortController();
		streamController = controller;

		let aiIdx = -1;
		const newToolCards = new Map(toolCards);

		// 45-second inactivity timeout — aborts if no new text or tool events arrive
		let lastActivityTime = Date.now();
		const timeoutId = setInterval(() => {
			if (Date.now() - lastActivityTime > 45_000) {
				controller.abort();
			}
		}, 5_000);

		// ── Word-level streaming buffer ─────────────────────────────────────
		// Incoming chars accumulate in wordAccum. On each animation frame we
		// flush complete words (split at whitespace) as individual .tok spans
		// so CSS can animate each word in. Partial last word stays buffered.
		let wordAccum = '';
		let wid = 0;
		let rafId: number | null = null;

		function flushWords(final = false) {
			if (!wordAccum || aiIdx < 0 || messages[aiIdx]?.role !== 'ai') { rafId = null; return; }
			let take = wordAccum;
			let keep = '';
			if (!final) {
				const cut = wordAccum.search(/\S+$/);
				if (cut > 0) { take = wordAccum.slice(0, cut); keep = wordAccum.slice(cut); }
				else if (cut === 0) { rafId = null; return; }
			}
			wordAccum = keep;
			const tokens = take.split(/(\s+)/).filter(s => s.length > 0);
			const newWords = tokens.map((t, i) => ({ t, id: wid++, delay: i * 22 }));
			const m = messages[aiIdx];
			messages[aiIdx] = { ...m, text: m.text + take, words: [...(m.words ?? []), ...newWords] };
			messages = [...messages];
			scrollIfNear();
			rafId = null;
		}

		function scheduleFlush() {
			if (!rafId) rafId = requestAnimationFrame(() => flushWords());
		}

		try {
			messages = messages.filter((m) => m.role !== 'thinking');

			for await (const delta of apiStreamChat({
				message: prompt,
				threadId: activeThreadId,
				workspaceNodeId: selectedNodeId,
				signal: controller.signal,
			})) {
				lastActivityTime = Date.now();

				if (delta.kind === 'text') {
					if (aiIdx < 0 || messages[aiIdx]?.role !== 'ai') {
						// Remove any post-tool thinking row before starting round-2 text
						messages = messages.filter((m) => m.role !== 'thinking');
						messages = [...messages, { role: 'ai', text: '', words: [], streaming: true }];
						aiIdx = messages.length - 1;
					}
					wordAccum += delta.content;
					scheduleFlush();
				} else if (delta.kind === 'tool_start') {
					newToolCards.set(delta.id, { name: delta.name, status: 'running', result: '', startTime: performance.now() });
					toolCards = new Map(newToolCards);
					aiIdx = -1;
				} else if (delta.kind === 'tool_result') {
					const card = newToolCards.get(delta.tool_use_id);
					if (card) {
						let isError = false;
						try { const obj = JSON.parse(delta.result); if (obj?.error || obj?.status === 'error') isError = true; } catch {}
						if (typeof delta.result === 'string' && delta.result.startsWith('Error:')) isError = true;
						newToolCards.set(delta.tool_use_id, { ...card, status: isError ? 'error' : 'success', result: delta.result });
						toolCards = new Map(newToolCards);
					}
					// Show thinking row while waiting for round-2 LLM response
					messages = [...messages.filter((m) => m.role !== 'thinking'), { role: 'thinking', text: '' }];
					aiIdx = -1;
				} else if (delta.kind === 'thread_id') {
					const tid = delta.id;
					if (tid !== activeThreadId) {
						activeThreadId = tid;
						// Prepend to recents sidebar (trim duplicates)
						const title = prompt.slice(0, 60) + (prompt.length > 60 ? '…' : '');
						recents = [{ id: tid, title }, ...recents.filter((r) => r.id !== tid)].slice(0, 20);
						try { localStorage.setItem('conusai-recents', JSON.stringify(recents)); } catch { /* ignore */ }
					}
				}
				// 'done' terminates the generator naturally
			}

			// Final flush of any remaining partial word
			if (rafId) { cancelAnimationFrame(rafId); rafId = null; }
			flushWords(true);
			// Stream done — clear word tokens, keep plain text, remove indicator
			if (aiIdx >= 0) messages[aiIdx] = { ...messages[aiIdx], streaming: false, words: undefined };
			messages = [...messages];
			// If this chat was bound to a workspace node, refresh its metadata so
			// the next selectNode call will find the newly-created thread_id.
			if (selectedNodeId) await refreshNodeMetadata(selectedNodeId);
		} catch (e: unknown) {
			clearInterval(timeoutId);
			messages = messages.filter((m) => m.role !== 'thinking');
			if (e instanceof Error && (e.name === 'AbortError' || e.message.includes('aborted'))) {
				// User navigated away or timed out — don't show an error toast
				if (messages.at(-1)?.role !== 'ai' || !messages.at(-1)?.text) {
					messages = [...messages, { role: 'ai', text: 'Request cancelled.' }];
				}
			} else {
				const msg = e instanceof Error ? e.message : String(e);
				messages = [...messages, { role: 'ai', text: `Stream failed: ${msg}` }];
				toasts.error(`Connection error: ${msg}`);
			}
		} finally {
			clearInterval(timeoutId);
			streamController = null;
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
			const result = await apiCall<InvoiceData>(fetch, EP.UI_EXTRACT_INVOICE, {
				method: 'POST', body: JSON.stringify({ token })
			});
			messages = messages.filter((_, i) => i !== loadIdx);
			if (result.error) { messages = [...messages, { role: 'ai', text: `Extraction failed: ${result.error.message}` }]; return; }
			invoiceResults = new Map([...invoiceResults, [token, result.data]]);
			messages = [...messages, { role: 'ai', text: '__invoice__' + token }];
		} catch (e: unknown) {
			messages = messages.filter((_, i) => i !== loadIdx);
			messages = [...messages, { role: 'ai', text: `Error: ${e instanceof Error ? e.message : String(e)}` }];
		} finally { inFlight = false; }
	}

	// ── Upload ────────────────────────────────────────────────────────────────
	const ALLOWED_MIME = new Set([
		'image/png', 'image/jpeg', 'image/webp', 'image/gif',
		'application/pdf', 'text/plain', 'text/markdown',
		'application/json', 'text/csv',
	]);
	const MAX_FILE_SIZE = 20 * 1024 * 1024;  // 20 MB
	const MAX_ATTACHMENTS = 5;

	async function uploadFiles(files: File[]) {
		for (const file of files) {
			// Attachment count limit
			if (pendingAttachments.length >= MAX_ATTACHMENTS) {
				toasts.warning(`Max ${MAX_ATTACHMENTS} attachments per message.`);
				break;
			}
			// MIME type validation
			if (!ALLOWED_MIME.has(file.type)) {
				toasts.error(`"${file.name}" — unsupported file type (${file.type || 'unknown'}).`);
				continue;
			}
			// Size limit
			if (file.size > MAX_FILE_SIZE) {
				toasts.error(`"${file.name}" is too large (max 20 MB).`);
				continue;
			}
			const result = await workspacesApi.uploadFile(fetch, file);
			if (result.error) {
				toasts.error(`Upload failed: ${result.error.message}`);
				continue;
			}
			const d = result.data;
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

		// If the message looks like an extract request and there are invoice attachments,
		// route each invoice through the extraction pipeline instead of the LLM.
		const isExtractIntent = /extract\s*invoice|invoice\s*extract/i.test(val) || (!val && pendingAttachments.every(isInvoice));
		const invoiceAttachments = isExtractIntent ? pendingAttachments.filter(isInvoice) : [];
		const chatAttachments = pendingAttachments.filter((a) => !invoiceAttachments.includes(a));

		inputValue = '';
		pendingAttachments = [];

		for (const a of invoiceAttachments) {
			extractInvoice(a.id, a.filename);
		}

		if (!chatAttachments.length && !val && invoiceAttachments.length) return;
		if (!val && !chatAttachments.length) return;

		let prompt = val;
		if (chatAttachments.length) {
			const origin = window.location.origin;
			const lines = chatAttachments.map((a) => `- ${a.filename} (image_path: ${origin}/ui/files/${a.id})`).join('\n');
			prompt = `${val}\n\n[Attached files]\n${lines}`;
		}
		streamChat(prompt);
	}

	// autoGrow is a Svelte action imported from $lib/ui/actions — use:autoGrow on the textarea

	// ── Load thread history ───────────────────────────────────────────────────
	async function loadThread(threadId: string) {
		if (inFlight) return;
		showChat = true;
		activeThreadId = threadId;
		messages = [{ role: 'ai', text: 'Loading…', streaming: true }];
		try {
			type MsgArr = { role: string; content: string }[];
			type MsgEnv = MsgArr | { data?: MsgArr; messages?: MsgArr; items?: MsgArr };
			const result = await apiCall<MsgEnv>(fetch, EP.THREAD_MESSAGES(threadId));
			if (result.error) { messages = [{ role: 'ai', text: 'Could not load thread.' }]; return; }
			const raw = result.data;
			const arr: MsgArr = Array.isArray(raw) ? raw
				: ((raw as { data?: MsgArr })?.data
					?? (raw as { messages?: MsgArr })?.messages
					?? (raw as { items?: MsgArr })?.items
					?? []);
			const filtered = arr.filter((m) => m.role === 'user' || m.role === 'assistant');
			messages = filtered.length
				? filtered.map((m) => ({ role: (m.role === 'user' ? 'user' : 'ai') as 'user' | 'ai', text: m.content }))
				: [{ role: 'ai', text: 'No messages in this thread yet.' }];
		} catch { messages = [{ role: 'ai', text: 'Failed to load thread.' }]; }
	}

	function newChat() {
		streamController?.abort();
		streamController = null;
		showChat = false;
		messages = [];
		activeThreadId = null;
		toolCards = new Map();
		inFlight = false;
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
</svelte:head>

<div class="app">
	<!-- ── Sidebar ── -->
	<aside class="sidebar" class:open={sidebarOpen} aria-label="Workshop navigation">
		<section class="nav-section ws-section" aria-labelledby="ws-heading">
			<header class="nav-header">
				<span id="ws-heading" class="nav-heading label-mono">Workspace</span>
				<button type="button" class="icon-btn ws-new-btn" aria-label="New folder or conversation"
					onclick={() => openNewNodeForm(selectedFolderParent())}>
					<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
						<line x1="9" y1="3" x2="9" y2="15"/><line x1="3" y1="9" x2="15" y2="9"/>
					</svg>
				</button>
			</header>
			<div class="ws-search-wrap">
				<svg class="ws-search-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
					<circle cx="6.5" cy="6.5" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/>
				</svg>
				<input id="ws-search" class="ws-search-input" type="search" placeholder="Search conversations…"
					autocomplete="off" spellcheck="false" aria-label="Search workspace"
					value={searchQuery}
					oninput={onSearchInput}>
				{#if searchQuery}
					<button class="ws-search-clear" aria-label="Clear search" onclick={clearSearch}>
						<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
							<line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
						</svg>
					</button>
				{/if}
			</div>

			<!-- New folder / conversation form -->
			{#if showNewNodeForm}
				<form class="ws-new-form" onsubmit={submitNewNode}>
					<div class="ws-new-kind">
						<button type="button" class="ws-kind-btn" class:active={newNodeKind === 'folder'}
							onclick={() => newNodeKind = 'folder'}>📁 Folder</button>
						<button type="button" class="ws-kind-btn" class:active={newNodeKind === 'conversation'}
							onclick={() => newNodeKind = 'conversation'}>📄 Chat</button>
					</div>
					<div class="ws-new-row">
						<input class="ws-new-input" type="text" placeholder={newNodeKind === 'folder' ? 'Folder name…' : 'Conversation name…'}
							bind:value={newNodeName} use:focusInput maxlength={80} autocomplete="off" />
						<button type="submit" class="ws-new-ok" disabled={newNodeBusy} aria-label="Create">
							{#if newNodeBusy}…{:else}✓{/if}
						</button>
						<button type="button" class="ws-new-cancel" onclick={closeNewNodeForm} aria-label="Cancel">✕</button>
					</div>
					{#if newNodeError}<div class="ws-new-error">{newNodeError}</div>{/if}
				</form>
			{/if}

			<!-- Search results -->
			{#if searchQuery}
				<div class="ws-tree" role="listbox" aria-label="Search results">
					{#if searchResults.length === 0}
						<div class="empty-hint">No matches for "{searchQuery}"</div>
					{:else}
						{#each searchResults as node (node.id)}
							<button class="ws-node ws-node-{node.kind}" class:ws-node-selected={selectedNodeId === node.id}
								role="option" aria-selected={selectedNodeId === node.id}
								onclick={() => selectNode(node)}>
								<span class="ws-node-icon">{node.kind === 'folder' ? '📁' : '📄'}</span>
								<span class="ws-node-name">{node.name}</span>
								<span class="ws-node-path">{node.virtual_path}</span>
							</button>
						{/each}
					{/if}
				</div>
			{:else}
				<!-- Workspace tree -->
				<div id="workspace-tree" class="ws-tree" role="tree" aria-labelledby="ws-heading">
					{#if workspaceNodes.length === 0}
						<div class="empty-hint">No folders yet — click <strong>+</strong> to create one.</div>
					{:else}
						{#each workspaceNodes as node (node.id)}
							{@render treeNode(node, 0)}
						{/each}
					{/if}
				</div>
			{/if}
		</section>

		<div class="nav-section">
			<div class="nav-heading label-mono">Recents</div>
			<div class="recents-list" id="recents-list">
				{#each recents as r (r.id)}
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
			<a href="/logout" class="icon-btn" aria-label="Logout" data-sveltekit-reload>
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
					<h1 class="greeting-text">Good {data.user ? '' : ''}morning, {data.user?.firstName ?? 'there'}<span class="greeting-presence">{@render sonarDot(true)}</span></h1>
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
								{@render sonarDot()}
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
								{#if msg.streaming && msg.words}
									<span class="ai-text" aria-live="polite">{#each msg.words as w (w.id)}<span class="tok" style="animation-delay:{w.delay}ms">{w.t}</span>{/each}{#if msg.text}<span class="stream-cursor" aria-hidden="true"></span>{:else}&nbsp;{@render sonarDot(true)}{/if}</span>
								{:else}
									<span class="ai-text">{msg.text}</span>
								{/if}
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

					<!-- Waiting dot — appears directly after last message when idle -->
					{#if !inFlight}
						<div class="chat-end-dot">
							{@render sonarDot()}
						</div>
					{/if}
				</div>

				<div class="composer-bottom">
					{@render composer()}
				</div>
			</section>
		{/if}
	</main>
</div>

<!-- ── Sonar-ping dot — single component used everywhere ─────────────────── -->
{#snippet sonarDot(sm: boolean = false)}
	<span class="sonar" class:sonar-sm={sm} role="status" aria-label="Waiting">
		<span class="sonar-ring sonar-r1"></span>
		<span class="sonar-ring sonar-r2"></span>
		<span class="sonar-core"></span>
	</span>
{/snippet}

<!-- ── Composer snippet ────────────────────────────────────────────────────── -->
{#snippet composer()}
	<div class="composer-wrap">
		<form class="composer" class:drop-target={dropTarget}
			class:focused={composerFocused}
			class:has-content={inputValue.length > 0 || pendingAttachments.length > 0}
			aria-busy={inFlight}
			onsubmit={handleSubmit}
			onfocusin={() => (composerFocused = true)}
			onfocusout={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) composerFocused = false; }}
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
				rows="1" autocomplete="off" bind:value={inputValue}
				use:autoGrow
				onkeydown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); (e.currentTarget.closest('form') as HTMLFormElement)?.requestSubmit(); } }}></textarea>

			<input id="file-input" type="file" style="display:none" multiple
				onchange={(e) => { const files = e.currentTarget.files; if (files?.length) uploadFiles([...files]); e.currentTarget.value = ''; }}>

			<div class="composer-toolbar">
				<button type="button" class="toolbar-btn" aria-label="Attach file"
					onclick={() => document.getElementById('file-input')?.click()}>
					<svg class="icon" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
						<path d="M15 9l-6 6a4 4 0 0 1-5.657-5.657l7-7a2.5 2.5 0 0 1 3.536 3.536l-7 7a1 1 0 0 1-1.414-1.414l6-6"/>
					</svg>
				</button>
				<div class="toolbar-spacer"></div>
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

<!-- ── Workspace tree node snippet ─────────────────────────────────────────── -->
{#snippet treeNode(node: WorkspaceNode, depth: number)}
	{#if node.kind === 'folder'}
		<div class="ws-folder" style="--depth:{depth}">
			<button class="ws-node ws-node-folder" class:ws-node-expanded={expandedFolders.has(node.id)}
				class:ws-node-selected={selectedNodeId === node.id}
				onclick={() => { selectedNodeId = node.id; history.replaceState(null, '', `?ws=${node.id}`); toggleFolder(node); }}
				aria-expanded={expandedFolders.has(node.id)}>
				<span class="ws-node-chevron">{expandedFolders.has(node.id) ? '▾' : '▸'}</span>
				<span class="ws-node-icon">📁</span>
				<span class="ws-node-name">{node.name}</span>
			</button>
			{#if expandedFolders.has(node.id)}
				<div class="ws-children">
					{#if childNodes.has(node.id)}
						{#each childNodes.get(node.id) ?? [] as child (child.id)}
							{@render treeNode(child, depth + 1)}
						{/each}
					{:else}
						<div class="ws-loading">Loading…</div>
					{/if}
				</div>
			{/if}
		</div>
	{:else}
		<button class="ws-node ws-node-conversation" class:ws-node-selected={selectedNodeId === node.id}
			style="--depth:{depth}" onclick={() => selectNode(node)}>
			<span class="ws-node-icon">📄</span>
			<span class="ws-node-name">{node.name}</span>
		</button>
	{/if}
{/snippet}

