/**
 * Vitest spec: SSE reconnect / backoff state machine.
 * Verifies that streamChat retries on network failure with exponential backoff
 * and gives up after 3 attempts, and that { reconnect: false } disables retries.
 */
import { describe, it, expect, vi } from 'vitest';
import { streamChat } from '../lib/api/stream';

/** A mock fetch that fails `failCount` times then succeeds with one text event. */
function makeFlaky(failCount: number) {
	let calls = 0;
	return async (_url: string, _init?: RequestInit): Promise<Response> => {
		calls++;
		if (calls <= failCount) throw new Error(`Network error (call ${calls})`);
		const encoder = new TextEncoder();
		const stream = new ReadableStream({
			start(controller) {
				controller.enqueue(encoder.encode('data: {"choices":[{"delta":{"content":"recovered"}}]}\n\n'));
				controller.enqueue(encoder.encode('data: [DONE]\n\n'));
				controller.close();
			},
		});
		return new Response(stream, { status: 200, headers: { 'Content-Type': 'text/event-stream' } });
	};
}

async function collect(gen: AsyncGenerator<unknown>): Promise<unknown[]> {
	const results: unknown[] = [];
	for await (const item of gen) results.push(item);
	return results;
}

describe('streamChat reconnect', () => {
	it('retries and recovers after 1 failure', async () => {
		// Use fake timers so backoff sleeps don't slow the test
		vi.useFakeTimers();
		const flakyFetch = makeFlaky(1);
		const gen = streamChat({ message: 'hi', fetch: flakyFetch as typeof fetch });
		const promise = collect(gen);
		// Advance past the 200ms first-backoff delay
		await vi.advanceTimersByTimeAsync(300);
		const events = await promise;
		vi.useRealTimers();
		expect(events).toContainEqual({ kind: 'text', content: 'recovered' });
	});

	it('retries and recovers after 2 failures', async () => {
		vi.useFakeTimers();
		const flakyFetch = makeFlaky(2);
		const gen = streamChat({ message: 'hi', fetch: flakyFetch as typeof fetch });
		const promise = collect(gen);
		// Advance past 200ms + 600ms backoff
		await vi.advanceTimersByTimeAsync(1000);
		const events = await promise;
		vi.useRealTimers();
		expect(events).toContainEqual({ kind: 'text', content: 'recovered' });
	});

	it('throws after 3 failures (exhausts backoff list)', async () => {
		vi.useFakeTimers();
		const alwaysFails = async () => { throw new Error('permanent failure'); };
		const gen = streamChat({ message: 'hi', fetch: alwaysFails as unknown as typeof fetch });
		const promise = collect(gen);
		// Prevent unhandled rejection warning while fake timers fire
		promise.catch(() => {});
		// Advance past all backoffs: 200 + 600 + 1800 = 2600ms
		await vi.advanceTimersByTimeAsync(3000);
		vi.useRealTimers();
		await expect(promise).rejects.toThrow('permanent failure');
	});

	it('throws immediately on first failure when reconnect: false', async () => {
		const alwaysFails = async () => { throw new Error('no reconnect'); };
		const gen = streamChat(
			{ message: 'hi', fetch: alwaysFails as unknown as typeof fetch },
			{ reconnect: false }
		);
		await expect(collect(gen)).rejects.toThrow('no reconnect');
	});
});
