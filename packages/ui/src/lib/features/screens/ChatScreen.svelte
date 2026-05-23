<script lang="ts">
	import AgentChatStream from '../AgentChatStream.svelte';
	import AgentChatComposer from '../../components/AgentChatComposer.svelte';
	import SuggestionChips from '../SuggestionChips.svelte';
	import ContextChip from '../ContextChip.svelte';
	import CapabilityBrowser, { type CapEntry } from '../CapabilityBrowser.svelte';
	import AppBottomSheet from '../../components/AppBottomSheet.svelte';
	import type { Attachment } from '../../components/AgentChatComposer.svelte';
	import type { ConusSdk } from '@conusai/sdk';
	import type { WorkspaceNode } from '@conusai/types';
	import { recordRect, playFlip } from '../../motion/index.js';

	let {
		sdk,
		chatStream,
		selectedNode,
		onSelectNode,
		userName,
		sigil,
		suggestions,
	}: {
		sdk: ConusSdk;
		chatStream: {
			messages: any[];
			toolCards: Map<string, any>;
			toolCardsList?: Array<[string, any]>;
			inFlight: boolean;
			lastRoutingMeta?: any;
			lastInvalidation?: any;
			send: (p: string, opts?: any) => void;
			newSession: () => void;
		};
		selectedNode: WorkspaceNode | null;
		onSelectNode: (n: WorkspaceNode | null) => void;
		userName: string;
		/** Optional sigil image URL shown above the greeting in empty state. */
		sigil?: string;
		/** Optional override for the 4 suggestion chips. */
		suggestions?: string[];
	} = $props();

	let inputValue = $state('');
	let attachments = $state<Attachment[]>([]);
	let messagesEl = $state<HTMLElement | undefined>();
	let chipsUsed = $state(false);
	let retryPickerOpen = $state(false);

	function handleRetryWithCapability() {
		retryPickerOpen = true;
	}

	function handlePickCapabilityForRetry(cap: CapEntry) {
		retryPickerOpen = false;
		const last = chatStream.lastSend;
		if (!last) return;
		chatStream.send(last.prompt, {
			workspaceNodeId: last.workspaceNodeId,
			attachmentIds: last.attachmentIds,
			forcedCapability: cap.name,
		});
	}

	// Reset local composer state when chatStream.newSession() clears messages
	$effect(() => {
		if (chatStream.messages.length === 0) {
			inputValue = '';
			attachments = [];
			chipsUsed = false;
		}
	});

	// For FLIP transition between empty-state and active-state composers
	let centredComposerEl = $state<HTMLElement | undefined>();
	let dockedComposerEl = $state<HTMLElement | undefined>();
	let savedRect: DOMRect | null = null;

	const isEmpty = $derived(chatStream.messages.length === 0);

	const DEFAULT_SUGGESTIONS = [
		'What can you help me with?',
		'Explain the difference between AI agents and AI assistants.',
		'What tools and capabilities do you have?',
		'What is the current time?',
	];
	const SUGGESTIONS = $derived(suggestions ?? DEFAULT_SUGGESTIONS);

	function greeting() {
		const h = new Date().getHours();
		if (h < 12) return 'Good morning';
		if (h < 17) return 'Good afternoon';
		return 'Good evening';
	}

	const greetingWords = $derived(
		`${greeting()}, ${userName.split(' ')[0]}.`.split(' ')
	);

	function handleSubmit(prompt: string, atts: Attachment[] = []) {
		if (!prompt.trim() && atts.length === 0) return;
		if (centredComposerEl) savedRect = recordRect(centredComposerEl);
		chipsUsed = true;
		chatStream.send(prompt, {
			workspaceNodeId: selectedNode?.id,
			attachmentIds: atts.map(a => a.id),
		});
		inputValue = '';
		attachments = [];
	}

	function handleSuggestion(text: string) {
		chipsUsed = true;
		handleSubmit(text);
	}

	$effect(() => {
		if (!isEmpty && dockedComposerEl && savedRect) {
			const rect = savedRect;
			savedRect = null;
			requestAnimationFrame(() => {
				if (dockedComposerEl) playFlip(dockedComposerEl, rect, { duration: 320 });
			});
		}
	});

	async function handleUpload(files: File[]): Promise<Attachment[]> {
		const results: Attachment[] = [];
		for (const file of files) {
			const res = await sdk.workspaces.upload(file);
			if (res.data) {
				results.push({
					id: res.data.id,
					filename: res.data.filename,
					size: res.data.size,
				});
			}
		}
		return results;
	}
</script>

<div class="chat-screen">
	{#if isEmpty}
		<!-- Empty state: centred greeting + composer + suggestions -->
		<div class="empty-state">
			{#if sigil}
				<img class="sigil" src={sigil} alt="" aria-hidden="true" />
			{/if}

			<h1 class="greeting" aria-label="{greeting()}, {userName.split(' ')[0]}.">
				{#each greetingWords as word, i}
					<span class="word" style="animation-delay: {200 + i * 40}ms">{word}</span>
					{#if i < greetingWords.length - 1}<span class="word-space"> </span>{/if}
				{/each}
			</h1>

			<p class="sub">How can I help you today?</p>

			<div class="centred-composer" bind:this={centredComposerEl}>
				{#if selectedNode}
					<div class="context-row">
						<ContextChip node={selectedNode} onClear={() => onSelectNode(null)} />
					</div>
				{/if}
				<AgentChatComposer
					bind:value={inputValue}
					bind:attachments
					inFlight={chatStream.inFlight}
					onsubmit={handleSubmit}
					onUpload={handleUpload}
				/>
			</div>

			{#if !chipsUsed}
				<SuggestionChips suggestions={SUGGESTIONS} onSelect={handleSuggestion} />
			{/if}
		</div>
	{:else}
		<!-- Active chat state -->
		<div class="messages-area">
			<AgentChatStream
				messages={chatStream.messages}
				toolCards={chatStream.toolCards}
				toolCardsList={chatStream.toolCardsList}
				inFlight={chatStream.inFlight}
				routingMeta={chatStream.lastRoutingMeta ?? null}
				onRetryWithCapability={handleRetryWithCapability}
				bind:messagesEl
			/>
		</div>

		<div class="composer-dock" bind:this={dockedComposerEl}>
			{#if selectedNode}
				<div class="context-row context-row--docked">
					<ContextChip node={selectedNode} onClear={() => onSelectNode(null)} />
				</div>
			{/if}
			<AgentChatComposer
				bind:value={inputValue}
				bind:attachments
				inFlight={chatStream.inFlight}
				onsubmit={handleSubmit}
				onUpload={handleUpload}
			/>
		</div>
	{/if}

	<!-- Retry-with-capability picker (PR 3.B.2). Opens when the assistant turn
	     served zero tools or the LLM said it doesn't have the needed tool. -->
	{#if retryPickerOpen}
		<AppBottomSheet open onClose={() => (retryPickerOpen = false)}>
			{#snippet children()}
				<div class="picker-wrap">
					<h2 class="picker-title">Pick a capability</h2>
					<CapabilityBrowser {sdk} onSelect={handlePickCapabilityForRetry} />
				</div>
			{/snippet}
		</AppBottomSheet>
	{/if}
</div>

<style>
	.chat-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--paper);
	}

	/* ── Empty state ────────────────────────────────────────────────────── */
	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-5) var(--space-4) var(--space-4);
		gap: var(--space-3);
	}

	.sigil {
		width: 68px;
		height: 68px;
		border-radius: var(--radius-lg);
		background: var(--paper-2);
		object-fit: contain;
		animation: sigil-enter var(--duration-slow) var(--ease-spring) both;
	}
	@keyframes sigil-enter {
		0%   { opacity: 0; transform: scale(0.72) rotate(-8deg); filter: blur(4px); }
		60%  { opacity: 1; transform: scale(1.05) rotate(1deg);  filter: blur(0); }
		100% { opacity: 1; transform: scale(1)    rotate(0deg);  filter: blur(0); }
	}

	.greeting {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-h1);
		font-weight: 700;
		letter-spacing: -1px;
		line-height: 1.05;
		color: var(--ink);
		text-align: center;
		margin: 0;
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 0.25em;
	}
	.word {
		display: inline-block;
		opacity: 0;
		animation: word-in var(--duration-stagger) var(--ease-out) both;
	}
	.word-space { display: none; }
	@keyframes word-in {
		from { opacity: 0; transform: translateY(6px); }
		to   { opacity: 1; transform: translateY(0); }
	}

	.sub {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		color: var(--ink-2);
		text-align: center;
		margin: 0;
		animation: fade-up var(--duration-stagger) var(--ease-out) 380ms both;
	}
	@keyframes fade-up {
		from { opacity: 0; transform: translateY(6px); }
		to   { opacity: 1; transform: translateY(0); }
	}

	@media (prefers-reduced-motion: reduce) {
		.sigil { animation: none; opacity: 1; }
		.word  { animation: none; opacity: 1; }
		.sub   { animation: none; opacity: 1; }
	}

	.centred-composer {
		width: 100%;
		max-width: var(--composer-w, 720px);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		animation: fade-up var(--duration-stagger) var(--ease-out) 420ms both;
	}
	@media (prefers-reduced-motion: reduce) {
		.centred-composer { animation: none; opacity: 1; }
	}

	.context-row {
		display: flex;
		justify-content: center;
	}

	/* ── Active state ───────────────────────────────────────────────────── */
	.messages-area {
		/* Flex passthrough — must NOT be the scroll container.
		 * AgentChatStream's inner .messages div has overflow-y:auto and
		 * calls scrollTo() on itself; if this wrapper also scrolls, the
		 * inner element never overflows and scrollToBottom() is a no-op. */
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.composer-dock {
		flex-shrink: 0;
		border-top: 1px solid var(--rule);
		background: var(--paper);
		padding-bottom: env(safe-area-inset-bottom);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding-top: var(--space-2);
	}

	.context-row--docked {
		padding: 0 var(--space-4);
		justify-content: flex-start;
	}

	.picker-wrap {
		padding: var(--space-3) 0;
		max-height: 70vh;
		overflow-y: auto;
	}
	.picker-title {
		margin: 0 0 var(--space-2);
		padding: 0 var(--space-4);
		font-size: var(--t-h3);
		font-weight: 600;
		color: var(--ink);
	}
</style>
