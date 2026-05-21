import type { ChatStreamDelta, ConusSdk } from '@conusai/sdk';
import type { StreamChatParams } from '@conusai/sdk';

export interface ChatMessage {
  role: 'user' | 'ai' | 'thinking';
  text: string;
  streaming?: boolean;
  words?: { t: string; id: number; delay: number }[];
}

export interface ToolCardEntry {
  name: string;
  status: 'running' | 'success' | 'error';
  result: string;
  startTime: number;
}

const INACTIVITY_TIMEOUT_MS = 45_000;
let wordSeq = 0;

export type CustomStreamFn = (params: StreamChatParams) => AsyncGenerator<ChatStreamDelta>;

export function createChatStream(sdk: ConusSdk, options?: { streamFn?: CustomStreamFn }) {
  let messages = $state<ChatMessage[]>([]);
  let toolCards = $state(new Map<string, ToolCardEntry>());
  // KNOWN BUG (2026-05-21): a `$state` Map exposed via the plain-object getter
  // below doesn't propagate `Map.set` mutations reactively to child component
  // templates (`{#each [...toolCards.entries()]}` in <AgentChatStream> never
  // re-runs even when the source-side `.size` becomes ≥ 1).  We bump
  // `toolCardsVersion` to give consumers a tracked $state read in the same
  // expression, AND we also publish a pre-flattened `toolCardsList` array.
  // Neither alone is sufficient — keep both until the underlying tracking
  // gap is resolved.  Tracked under §17 in docs/verify/verify-ios.md.
  let toolCardsVersion = $state(0);
  let inFlight = $state(false);
  let activeThreadId = $state<string | null>(null);
  let controller: AbortController | null = null;

  function abort() {
    controller?.abort();
    controller = null;
    inFlight = false;
  }

  function newSession() {
    abort();
    messages = [];
    toolCards.clear();
    toolCardsVersion++;
    activeThreadId = null;
  }

  async function send(
    prompt: string,
    opts: { workspaceNodeId?: string | null; attachmentIds?: string[]; onThreadId?: (id: string) => void } = {}
  ) {
    if (inFlight || !prompt.trim()) return;
    inFlight = true;
    controller = new AbortController();
    const signal = controller.signal;

    messages = [...messages, { role: 'user', text: prompt }];

    let aiIdx = -1;
    let wordAccum = '';
    let rafId: number | null = null;
    let lastActivityTime = Date.now();

    const timeoutId = setInterval(() => {
      if (Date.now() - lastActivityTime > INACTIVITY_TIMEOUT_MS) { abort(); }
    }, 5000);

    function flushWords(final = false) {
      rafId = null;
      if (!wordAccum && !final) return;
      const raw = wordAccum;
      wordAccum = '';
      if (aiIdx < 0) return;
      const chunks = raw.match(/\S+\s*/g) ?? [raw];
      const existing = messages[aiIdx].words ?? [];
      const added = chunks.map((t, i) => ({ t, id: wordSeq++, delay: i * 18 }));
      messages[aiIdx] = {
        ...messages[aiIdx],
        text: (messages[aiIdx].text ?? '') + raw,
        words: [...existing, ...added],
      };
      messages = [...messages];
    }

    function scheduleFlush() {
      if (!rafId) rafId = requestAnimationFrame(() => flushWords());
    }

    try {
      messages = [...messages.filter(m => m.role !== 'thinking'), { role: 'thinking', text: '' }];

      const streamParams: StreamChatParams = {
        message: prompt,
        threadId: activeThreadId,
        workspaceNodeId: opts.workspaceNodeId,
        attachmentIds: opts.attachmentIds,
        signal,
      };
      const gen = options?.streamFn
        ? options.streamFn(streamParams)
        : sdk.chat.stream(streamParams);

      for await (const delta of gen) {
        lastActivityTime = Date.now();

        if (delta.kind === 'text') {
          if (aiIdx < 0 || messages[aiIdx]?.role !== 'ai') {
            messages = messages.filter(m => m.role !== 'thinking');
            messages = [...messages, { role: 'ai', text: '', words: [], streaming: true }];
            aiIdx = messages.length - 1;
          }
          wordAccum += delta.content;
          scheduleFlush();
        } else if (delta.kind === 'tool_start') {
          const next = new Map(toolCards);
          next.set(delta.id, {
            name: delta.name,
            status: 'running',
            result: '',
            startTime: performance.now(),
          });
          toolCards = next;
          toolCardsVersion++;
          aiIdx = -1;
        } else if (delta.kind === 'tool_result') {
          const card = toolCards.get(delta.tool_use_id);
          if (card) {
            let isError = false;
            try {
              const o = JSON.parse(delta.result);
              if (o?.error || o?.status === 'error') isError = true;
            } catch {}
            if (delta.result.startsWith('Error:')) isError = true;
            const next = new Map(toolCards);
            next.set(delta.tool_use_id, {
              ...card,
              status: isError ? 'error' : 'success',
              result: delta.result,
            });
            toolCards = next;
            toolCardsVersion++;
          }
          messages = [...messages.filter(m => m.role !== 'thinking'), { role: 'thinking', text: '' }];
          aiIdx = -1;
        } else if (delta.kind === 'thread_id') {
          if (delta.id !== activeThreadId) {
            activeThreadId = delta.id;
            opts.onThreadId?.(delta.id);
          }
        }
      }

      if (rafId) { cancelAnimationFrame(rafId); rafId = null; }
      flushWords(true);
      if (aiIdx >= 0) messages[aiIdx] = { ...messages[aiIdx], streaming: false, words: undefined };
      messages = [...messages];
    } catch (e: unknown) {
      messages = messages.filter(m => m.role !== 'thinking');
      if (e instanceof Error && (e.name === 'AbortError' || e.message.includes('aborted'))) {
        // If messages was cleared by newSession(), don't add "Request cancelled." — the session was intentionally reset.
        if (messages.length > 0 && (messages.at(-1)?.role !== 'ai' || !messages.at(-1)?.text)) {
          messages = [...messages, { role: 'ai', text: 'Request cancelled.' }];
        }
      } else if (messages.length > 0) {
        messages = [...messages, { role: 'ai', text: `Stream failed: ${e instanceof Error ? e.message : String(e)}` }];
      }
    } finally {
      clearInterval(timeoutId);
      controller = null;
      inFlight = false;
    }
  }

  async function loadThread(sdk: ConusSdk, threadId: string) {
    inFlight = true;
    activeThreadId = threadId;
    messages = [{ role: 'ai', text: 'Loading…', streaming: true }];
    try {
      const result = await sdk.threads.messages(threadId);
      if (result.error) { messages = [{ role: 'ai', text: 'Could not load thread.' }]; return; }
      type Msg = { role: string; content: string };
      const arr = result.data as Msg[];
      const filtered = arr.filter(m => m.role === 'user' || m.role === 'assistant');
      messages = filtered.length
        ? filtered.map(m => ({ role: (m.role === 'user' ? 'user' : 'ai') as 'user' | 'ai', text: m.content }))
        : [{ role: 'ai', text: 'No messages yet.' }];
    } catch { messages = [{ role: 'ai', text: 'Failed to load thread.' }]; }
    finally { inFlight = false; }
  }

  // Expose the closed-over $state via getters that touch a version counter
  // in the SAME reactive scope. The trick is to also expose `toolCardsList`
  // — a plain array derived from the Map — so consumers can iterate without
  // crossing the plain-object factory boundary that breaks Map prop tracking
  // (observed 2026-05-21: `Map.set` updated `.size` on the source side but
  // `{#each [...toolCards.entries()]}` in `<AgentChatStream>` never re-ran).
  const api = {
    get messages() { return messages; },
    get toolCards() {
      void toolCardsVersion;
      return toolCards;
    },
    get toolCardsList(): Array<[string, ToolCardEntry]> {
      void toolCardsVersion;
      return Array.from(toolCards.entries());
    },
    get inFlight() { return inFlight; },
    get activeThreadId() { return activeThreadId; },
    send,
    loadThread: (threadId: string) => loadThread(sdk, threadId),
    newSession,
    abort,
  };
  return api;
}
