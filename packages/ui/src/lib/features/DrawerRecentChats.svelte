<script lang="ts">
	/**
	 * Recent chats list — live-refreshes via `createLiveResource('threads', …)`
	 * whenever the gateway emits a `resource_invalidated` for `threads` (PR 3.A.6).
	 *
	 * Cross-app: lives in `packages/ui`, consumed identically by `apps/web` and
	 * `apps/browser-shell`. No app-local copy is permitted (§0.5 parity invariant,
	 * enforced by `scripts/check-cross-app-imports.mjs`).
	 *
	 * The component owns the fetcher + invalidation forwarding; consumers pass
	 * `sdk`, `tenantId` (for the defensive scope check), the active `chatStream`
	 * (so the SSE delta drives refresh), and an `onSelect` callback.
	 */
	import type { ConusSdk, ThreadSummary } from '@conusai/sdk';
	import { createLiveResource } from '../live/createLiveResource.svelte.js';
	import type { createChatStream } from './createChatStream.svelte.js';

	let {
		sdk,
		tenantId,
		chatStream,
		onSelect,
		limit = 20,
	}: {
		sdk: ConusSdk;
		/** Tenant id from the SvelteKit session. Pass `null` for shells without a server-side session. */
		tenantId: string | null;
		/** The `createChatStream` instance whose `lastInvalidation` we forward to the resource. */
		chatStream: ReturnType<typeof createChatStream>;
		onSelect: (thread: { id: string; title: string }) => void;
		limit?: number;
	} = $props();

	const recents = createLiveResource<ThreadSummary[]>(
		'threads',
		async () => {
			const res = await sdk.threads.list({ limit });
			if (res.error) return [];
			return res.data ?? [];
		},
		{ tenantId },
	);

	// Forward chat-stream invalidations to the live resource. The factory's own
	// $effect tracks the SDK fetch on mount; this $effect tracks the SSE delta
	// stream and triggers a re-fetch when the gateway flagged threads as dirty.
	$effect(() => {
		const inv = chatStream.lastInvalidation;
		if (inv) recents.notifyInvalidationWithScope(inv.resource, inv.scope);
	});

	function timeAgo(ts: string | undefined): string {
		if (!ts) return '';
		const diff = (Date.now() - new Date(ts).getTime()) / 1000;
		if (diff < 60) return 'just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		return `${Math.floor(diff / 86400)}d ago`;
	}
</script>

<section class="recents-section">
	<div class="section-header">
		<span class="section-label">RECENT</span>
	</div>

	{#if recents.data === null && recents.loading}
		<div class="state-row loading" aria-live="polite" aria-label="Loading recent chats">
			<svg class="spinner" viewBox="0 0 24 24" fill="none" stroke="currentColor"
				stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
				width="16" height="16" aria-hidden="true">
				<path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
			</svg>
		</div>
	{:else if recents.data === null && recents.error}
		<div class="state-row error" aria-live="polite">Could not load recents.</div>
	{:else if !recents.data || recents.data.length === 0}
		<div class="state-row">No recent chats yet.</div>
	{:else}
		{#each recents.data as thread (thread.id)}
			<button
				class="recent-row"
				onclick={() => onSelect({ id: thread.id, title: thread.title ?? 'Untitled' })}
			>
				<svg
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="1.75"
					stroke-linecap="round"
					stroke-linejoin="round"
					width="16"
					height="16"
					class="recent-icon"
					aria-hidden="true"
				>
					<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
				</svg>
				<span class="recent-title">{thread.title ?? 'Untitled'}</span>
				<span class="recent-time">{timeAgo(thread.last_active)}</span>
			</button>
		{/each}
	{/if}
</section>

<style>
	.recents-section {
		border-top: 1px solid var(--rule);
		display: flex;
		flex-direction: column;
	}

	.section-header {
		padding: var(--space-2) var(--space-3) var(--space-1) var(--space-4);
	}

	.section-label {
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 500;
		letter-spacing: 0.14em;
		color: var(--ink-3);
		text-transform: uppercase;
	}

	.state-row {
		padding: var(--space-2) var(--space-4) var(--space-3);
		font-family: var(--font-family-sans);
		font-size: 13px;
		color: var(--ink-3);
	}
	.state-row.loading {
		display: flex;
		align-items: center;
	}
	.state-row.error {
		color: var(--danger, #c44);
	}
	.spinner {
		color: var(--ink-3);
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
	@media (prefers-reduced-motion: reduce) {
		.spinner { animation: none; }
	}

	.recent-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: 44px;
		padding: 0 var(--space-4);
		border: none;
		background: none;
		cursor: pointer;
		width: 100%;
		text-align: left;
		transition: background var(--duration-fast);
	}

	.recent-row:hover {
		background: var(--paper-3);
	}

	.recent-icon {
		color: var(--ink-3);
		flex-shrink: 0;
	}

	@media (prefers-reduced-motion: reduce) {
		.recent-row {
			transition: none;
		}
	}

	.recent-title {
		flex: 1;
		font-family: var(--font-family-sans);
		font-size: 15px;
		color: var(--ink);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.recent-time {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--ink-3);
		flex-shrink: 0;
	}
</style>
