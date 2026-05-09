/**
 * Vitest spec: SSE parser — feeds a mock ReadableStream to streamChat and
 * asserts the yielded ChatStreamDelta sequence matches the byte stream.
 *
 * Tests the core SSE parsing logic inside src/lib/api/stream.ts without
 * a live backend. Reconnect is disabled via { reconnect: false }.
 */
import { describe, it, expect } from 'vitest';
import { streamChat } from '../lib/api/stream';

/** Build a mock fetch that returns one SSE response from the given chunks. */
function mockFetch(chunks: string[]) {
	return async (_url: string, _init?: RequestInit): Promise<Response> => {
		const encoder = new TextEncoder();
		const stream = new ReadableStream({
			start(controller) {
				for (const chunk of chunks) {
					controller.enqueue(encoder.encode(chunk));
				}
				controller.close();
			},
		});
		return new Response(stream, {
			status: 200,
			headers: { 'Content-Type': 'text/event-stream' },
		});
	};
}

async function collect(gen: AsyncGenerator<unknown>): Promise<unknown[]> {
	const results: unknown[] = [];
	for await (const item of gen) results.push(item);
	return results;
}

describe('streamChat SSE parser', () => {
	it('yields text deltas from a simple text stream', async () => {
		const chunks = [
			'data: {"choices":[{"delta":{"content":"Hello"}}]}\n\n',
			'data: {"choices":[{"delta":{"content":" world"}}]}\n\n',
			'data: [DONE]\n\n',
		];
		const gen = streamChat({ message: 'hi', fetch: mockFetch(chunks) as typeof fetch }, { reconnect: false });
		const events = await collect(gen);
		expect(events).toEqual([
			{ kind: 'text', content: 'Hello' },
			{ kind: 'text', content: ' world' },
			{ kind: 'done' },
		]);
	});

	it('yields tool_start and tool_result events', async () => {
		const chunks = [
			'data: {"choices":[{"delta":{"tool_call_start":{"id":"tc-1","name":"invoice-processing__extract_invoice"}}}]}\n\n',
			'data: {"choices":[{"delta":{"tool_call_result":{"tool_use_id":"tc-1","result":"{\\"invoice_number\\":\\"HCY-123\\"}"}}}]}\n\n',
			'data: [DONE]\n\n',
		];
		const gen = streamChat({ message: 'extract', fetch: mockFetch(chunks) as typeof fetch }, { reconnect: false });
		const events = await collect(gen);
		expect(events).toEqual([
			{ kind: 'tool_start', id: 'tc-1', name: 'invoice-processing__extract_invoice' },
			{ kind: 'tool_result', tool_use_id: 'tc-1', result: '{"invoice_number":"HCY-123"}' },
			{ kind: 'done' },
		]);
	});

	it('yields thread_id when present in event', async () => {
		const chunks = [
			'data: {"thread_id":"tid-abc","choices":[{"delta":{"content":"Hi"}}]}\n\n',
			'data: [DONE]\n\n',
		];
		const gen = streamChat({ message: 'hi', fetch: mockFetch(chunks) as typeof fetch }, { reconnect: false });
		const events = await collect(gen);
		expect(events).toContainEqual({ kind: 'thread_id', id: 'tid-abc' });
		expect(events).toContainEqual({ kind: 'text', content: 'Hi' });
	});

	it('handles chunks split mid-event (partial SSE frames)', async () => {
		// Simulate the stream arriving in byte chunks that don't align to event boundaries
		const chunks = [
			'data: {"choices":[{"delta":{"cont',
			'ent":"Split"}}]}\n\n',
			'data: [DONE]\n\n',
		];
		const gen = streamChat({ message: 'hi', fetch: mockFetch(chunks) as typeof fetch }, { reconnect: false });
		const events = await collect(gen);
		expect(events).toEqual([
			{ kind: 'text', content: 'Split' },
			{ kind: 'done' },
		]);
	});

	it('skips malformed JSON lines without throwing', async () => {
		const chunks = [
			'data: not-json\n\n',
			'data: {"choices":[{"delta":{"content":"OK"}}]}\n\n',
			'data: [DONE]\n\n',
		];
		const gen = streamChat({ message: 'hi', fetch: mockFetch(chunks) as typeof fetch }, { reconnect: false });
		const events = await collect(gen);
		expect(events).toEqual([
			{ kind: 'text', content: 'OK' },
			{ kind: 'done' },
		]);
	});
});
