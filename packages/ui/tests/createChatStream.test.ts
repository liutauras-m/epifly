import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createChatStream } from '../src/lib/features/createChatStream.svelte.js';
import type { ConusSdk } from '@conusai/sdk';

function makeSdk(deltas: any[]): ConusSdk {
  return {
    chat: {
      stream: vi.fn(async function* () {
        for (const d of deltas) yield d;
      }),
    },
  } as unknown as ConusSdk;
}

describe('createChatStream', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    global.requestAnimationFrame = vi.fn((cb) => { cb(0); return 0; }) as any;
    global.cancelAnimationFrame = vi.fn();
    global.performance = { now: vi.fn(() => 0) } as any;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('starts with empty messages and inFlight=false', () => {
    const cs = createChatStream(makeSdk([]));
    expect(cs.messages).toHaveLength(0);
    expect(cs.inFlight).toBe(false);
  });

  it('appends user message immediately on send', async () => {
    const sdk = makeSdk([]);
    const cs = createChatStream(sdk);
    const p = cs.send('hello');
    expect(cs.messages[0]).toMatchObject({ role: 'user', text: 'hello' });
    await p;
  });

  it('accumulates text deltas into ai message', async () => {
    const sdk = makeSdk([
      { kind: 'text', content: 'Hello ' },
      { kind: 'text', content: 'world' },
    ]);
    const cs = createChatStream(sdk);
    await cs.send('hi');
    const ai = cs.messages.find(m => m.role === 'ai');
    expect(ai?.text).toBe('Hello world');
  });

  it('tracks thread_id from stream delta', async () => {
    const onThreadId = vi.fn();
    const sdk = makeSdk([{ kind: 'thread_id', id: 'thread-abc' }]);
    const cs = createChatStream(sdk);
    await cs.send('hi', { onThreadId });
    expect(cs.activeThreadId).toBe('thread-abc');
    expect(onThreadId).toHaveBeenCalledWith('thread-abc');
  });

  it('does not call onThreadId twice for same thread', async () => {
    const onThreadId = vi.fn();
    const sdk = makeSdk([
      { kind: 'thread_id', id: 'thread-abc' },
      { kind: 'thread_id', id: 'thread-abc' },
    ]);
    const cs = createChatStream(sdk);
    await cs.send('hi', { onThreadId });
    expect(onThreadId).toHaveBeenCalledTimes(1);
  });

  it('marks tool cards running then resolved', async () => {
    const sdk = makeSdk([
      { kind: 'tool_start', id: 'tc-1', name: 'web_search' },
      { kind: 'tool_result', tool_use_id: 'tc-1', result: '{"ok":true}' },
    ]);
    const cs = createChatStream(sdk);
    await cs.send('search something');
    expect(cs.toolCards.get('tc-1')).toMatchObject({ name: 'web_search', status: 'success' });
  });

  it('marks tool card as error when result starts with Error:', async () => {
    const sdk = makeSdk([
      { kind: 'tool_start', id: 'tc-2', name: 'code_exec' },
      { kind: 'tool_result', tool_use_id: 'tc-2', result: 'Error: timeout' },
    ]);
    const cs = createChatStream(sdk);
    await cs.send('run code');
    expect(cs.toolCards.get('tc-2')).toMatchObject({ status: 'error' });
  });

  it('newSession clears messages, toolCards, activeThreadId', async () => {
    const sdk = makeSdk([
      { kind: 'thread_id', id: 'thread-xyz' },
      { kind: 'text', content: 'hi' },
    ]);
    const cs = createChatStream(sdk);
    await cs.send('hello');
    cs.newSession();
    expect(cs.messages).toHaveLength(0);
    expect(cs.toolCards.size).toBe(0);
    expect(cs.activeThreadId).toBeNull();
  });

  it('sets inFlight=false after stream completes', async () => {
    const cs = createChatStream(makeSdk([{ kind: 'text', content: 'done' }]));
    await cs.send('go');
    expect(cs.inFlight).toBe(false);
  });

  it('ignores send while already inFlight', async () => {
    let resolve!: () => void;
    const sdk = {
      chat: {
        stream: vi.fn(async function* () {
          await new Promise<void>(r => { resolve = r; });
        }),
      },
    } as unknown as ConusSdk;
    const cs = createChatStream(sdk);
    cs.send('first');
    await cs.send('second');
    expect((sdk.chat.stream as any).mock.calls.length).toBe(1);
    resolve!();
  });
});
