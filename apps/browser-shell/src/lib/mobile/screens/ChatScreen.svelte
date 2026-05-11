<script lang="ts">
	import { AgentChatStream, AgentChatComposer } from '@conusai/ui/features';
	import type { Attachment } from '@conusai/ui/features';
	import type { ConusSdk } from '@conusai/sdk';
	import type { WorkspaceNode } from '@conusai/types';
	import favicon from '@conusai/ui/assets/images/favicon.png';
	import SuggestionChips from '../parts/SuggestionChips.svelte';
	import ContextChip from '../parts/ContextChip.svelte';
	import { recordRect, playFlip } from '../motion/flip.js';

	let {
		sdk,
		chatStream,
		selectedNode,
		onSelectNode,
		userName,
	}: {
		sdk: ConusSdk;
		chatStream: {
			messages: any[];
			toolCards: Map<string, any>;
			inFlight: boolean;
			send: (p: string, opts?: any) => void;
			newSession: () => void;
		};
		selectedNode: WorkspaceNode | null;
		onSelectNode: (n: WorkspaceNode | null) => void;
		userName: string;
	} = $props();

	let inputValue = $state('');
	let attachments = $state<Attachment[]>([]);
	let messagesEl = $state<HTMLElement | undefined>();
	let chipsUsed = $state(false);

	// For FLIP transition: bind to centred composer in empty state
	let centredComposerEl = $state<HTMLElement | undefined>();
	// For FLIP transition: bind to docked composer in active state
	let dockedComposerEl = $state<HTMLElement | undefined>();
	// Saved rect before state flip
	let savedRect: DOMRect | null = null;

	const isEmpty = $derived(chatStream.messages.length === 0);

	const SUGGESTIONS = [
		'What can you help me with?',
		'Explain the difference between AI agents and AI assistants.',
		'What tools and capabilities do you have?',
		'What is the current time?',
	];

	function greeting() {
		const h = new Date().getHours();
		if (h < 12) return 'Good morning';
		if (h < 17) return 'Good afternoon';
		return 'Good evening';
	}

	// Split greeting text into words for stagger animation
	const greetingWords = $derived(
		`${greeting()}, ${userName.split(' ')[0]}.`.split(' ')
	);

	function handleSubmit(prompt: string, atts: Attachment[] = []) {
		if (!prompt.trim() && atts.length === 0) return;
		// Record composer rect before state changes (for FLIP)
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

	// After state flips to active, play FLIP on docked composer
	$effect(() => {
		if (!isEmpty && dockedComposerEl && savedRect) {
			const rect = savedRect;
			savedRect = null;
			// Run after paint
			requestAnimationFrame(() => {
				if (dockedComposerEl) playFlip(dockedComposerEl, rect, { duration: 320 });
			});
		}
	});

	async function handleUpload(files: File[]): Promise<Attachment[]> {
		const results: Attachment[] = [];
		for (const file of files) {
			const res = await sdk.files.upload(file);
			if (res.data) {
				results.push({
					id: (res.data as any).token,
					filename: (res.data as any).name,
					size: (res.data as any).size_bytes,
				});
			}
		}
		return results;
	}
</script>

<div class="chat-screen">
	{#if isEmpty}
		<!-- Empty state: centred greeting -->
		<div class="empty-state">
			<img class="sigil" src={favicon} alt="" aria-hidden="true" />

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
				inFlight={chatStream.inFlight}
				bind:messagesEl
			/>
		</div>

		<div class="composer-dock" bind:this={dockedComposerEl}>
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

	/* ── Empty state ── */
	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--s-5) var(--s-4) var(--s-4);
		gap: var(--s-3);
	}

	/* Sigil: favicon image with enter + breathe animations */
	.sigil {
		width: 68px;
		height: 68px;
		border-radius: var(--r-lg);
		background: var(--paper-2);
		object-fit: contain;
		animation:
			sigil-enter 480ms cubic-bezier(0.05, 0.7, 0.1, 1) both,
			sigil-breathe 4s ease-in-out 1s infinite;
	}

	@keyframes sigil-enter {
		0%   { opacity: 0; transform: scale(0.72) rotate(-8deg); filter: blur(4px); }
		60%  { opacity: 1; transform: scale(1.05) rotate(1deg);  filter: blur(0); }
		100% { opacity: 1; transform: scale(1)    rotate(0deg);  filter: blur(0); }
	}

	@keyframes sigil-breathe {
		0%, 100% { transform: scale(1);    opacity: 0.92; }
		50%       { transform: scale(1.06); opacity: 1; }
	}

	/* Greeting: word-by-word stagger fade-up */
	.greeting {
		font-family: var(--font-display);
		font-size: 32px;
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
		animation: word-in 240ms cubic-bezier(0.22, 1, 0.36, 1) both;
	}

	.word-space {
		display: none; /* gap handles spacing */
	}

	@keyframes word-in {
		from { opacity: 0; transform: translateY(6px); }
		to   { opacity: 1; transform: translateY(0); }
	}

	.sub {
		font-family: var(--font-body);
		font-size: 17px;
		color: var(--ink-2);
		text-align: center;
		margin: 0;
		animation: fade-up 240ms cubic-bezier(0.22, 1, 0.36, 1) 380ms both;
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
		max-width: 640px;
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
		animation: fade-up 240ms cubic-bezier(0.22, 1, 0.36, 1) 420ms both;
	}

	@media (prefers-reduced-motion: reduce) {
		.centred-composer { animation: none; opacity: 1; }
	}

	/* ── Active state ── */
	.messages-area {
		flex: 1;
		overflow-y: auto;
		overflow-x: hidden;
	}

	.composer-dock {
		flex-shrink: 0;
		border-top: 1px solid var(--rule);
		background: var(--paper);
		padding-bottom: env(safe-area-inset-bottom);
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
		padding-top: var(--s-2);
	}

	.context-row {
		padding: 0 var(--s-4);
	}
</style>
