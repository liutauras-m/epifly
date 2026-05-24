import type { ChatStreamDelta, ConusSdk, RoutingMeta, StreamChatParams } from '@conusai/sdk';
import { toasts } from '../stores/toast.svelte.js';

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
  /** Public URL returned by host_project when hosting_type = "static". */
  publicUrl?: string;
  /** Workspace-relative project path extracted from the tool result. */
  projectPath?: string;
  /** Framework name extracted from the tool result metadata. */
  framework?: string;
}

const INACTIVITY_TIMEOUT_MS = 45_000;
let wordSeq = 0;

/** Stream function used by tests or custom gateways. `baseUrl` is NOT required — SDK fills it. */
export type CustomStreamFn = (params: Omit<StreamChatParams, 'baseUrl' | 'fetch'>) => AsyncGenerator<ChatStreamDelta>;

export interface CreateChatStreamOptions {
  streamFn?: CustomStreamFn;
  /**
   * Tenant identifier of the authenticated user. When set, `resource_invalidated`
   * deltas with a mismatching `scope` are dropped + logged (PR 3.A.7). This is a
   * defensive client-side guard — the gateway already filters by tenant before
   * sending — so production deployments that don't surface `tenantId` to the
   * client simply skip the check.
   */
  tenantId?: string | null;
}

export function createChatStream(sdk: ConusSdk, options?: CreateChatStreamOptions) {
  const tenantId = options?.tenantId ?? null;
  let messages = $state<ChatMessage[]>([]);
  let toolCards = $state(new Map<string, ToolCardEntry>());
  let lastRoutingMeta = $state<RoutingMeta | null>(null);
  /** Last `resource_invalidated` delta received from the gateway (PR 3.A). */
  let lastInvalidation = $state<{ resource: string; scope: string; changed_keys: string[] } | null>(null);
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
  /**
   * Last `send(...)` arguments, kept for the "Retry with explicit capability"
   * button (PR 3.B.2). Reset on `newSession()`.
   */
  let lastSend = $state<{
    prompt: string;
    workspaceNodeId?: string | null;
    attachmentIds?: string[];
    forcedCapability?: string | null;
  } | null>(null);

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
    lastSend = null;
  }

  async function send(
    prompt: string,
    opts: {
      workspaceNodeId?: string | null;
      attachmentIds?: string[];
      onThreadId?: (id: string) => void;
      /** Capability name to pin for this turn — set by "Invoke" button (PR 2.A). */
      forcedCapability?: string | null;
    } = {}
  ) {
    if (inFlight || !prompt.trim()) return;
    inFlight = true;
    controller = new AbortController();
    const signal = controller.signal;

    // Stash the call args so the "Retry with explicit capability" UI (PR 3.B.2)
    // can re-send with the same prompt + workspace node + attachments but a
    // different `forcedCapability`.
    lastSend = {
      prompt,
      workspaceNodeId: opts.workspaceNodeId,
      attachmentIds: opts.attachmentIds,
      forcedCapability: opts.forcedCapability,
    };

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

      const streamParams: Omit<StreamChatParams, 'baseUrl' | 'fetch'> = {
        message: prompt,
        threadId: activeThreadId,
        workspaceNodeId: opts.workspaceNodeId,
        attachmentIds: opts.attachmentIds,
        forcedCapability: opts.forcedCapability,
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
            let publicUrl: string | undefined;
            let projectPath: string | undefined;
            let framework: string | undefined;
            try {
              const o = JSON.parse(delta.result);
              if (o?.error || o?.status === 'error') isError = true;
              // Phase 9.4 — extract hosted-project metadata from tool result
              if (typeof o?.public_url === 'string') {
                publicUrl = o.public_url;
              }
              const meta = o?.metadata;
              if (meta) {
                if (typeof meta.root_path === 'string') projectPath = meta.root_path;
                if (typeof meta.framework === 'string') framework = meta.framework;
              }
            } catch {}
            if (delta.result.startsWith('Error:')) isError = true;
            if (isError) {
              const errMsg = delta.error ?? delta.result.replace(/^Error:\s*/, '');
              toasts.error(`Tool "${card.name.includes('__') ? card.name.split('__').pop()! : card.name}" failed: ${errMsg}`);
            }
            const next = new Map(toolCards);
            next.set(delta.tool_use_id, {
              ...card,
              status: isError ? 'error' : 'success',
              result: delta.result,
              ...(publicUrl  && { publicUrl }),
              ...(projectPath && { projectPath }),
              ...(framework  && { framework }),
            });
            toolCards = next;
            toolCardsVersion++;
          }
          messages = [...messages.filter(m => m.role !== 'thinking'), { role: 'thinking', text: '' }];
          aiIdx = -1;
        } else if (delta.kind === 'routing_meta') {
          lastRoutingMeta = {
            forced_capability: delta.forced_capability,
            selected_capabilities: delta.selected_capabilities,
            pinned_tools: delta.pinned_tools,
            lexical_hits: delta.lexical_hits,
            max_score: delta.max_score,
          };
        } else if (delta.kind === 'resource_invalidated') {
          // Defensive client-side scope check (PR 3.A.7). Drop + warn when the
          // configured tenantId doesn't match the event scope — server-side
          // filtering is the actual boundary, but a mismatch indicates either
          // misconfig or a real leak we want to expose loudly.
          if (tenantId != null && delta.scope !== tenantId) {
            console.warn(
              '[createChatStream] dropping cross-tenant invalidation: ' +
              `event scope "${delta.scope}" !== client tenantId "${tenantId}"`,
            );
          } else {
            lastInvalidation = {
              resource: delta.resource,
              scope: delta.scope,
              changed_keys: delta.changed_keys,
            };
          }
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
    /** Routing metadata from the most recent turn (PR 3.B). Null before first turn. */
    get lastRoutingMeta() { return lastRoutingMeta; },
    /** Last resource_invalidated event from the most recent turn (PR 3.A). Null until first mutation. */
    get lastInvalidation() { return lastInvalidation; },
    /** Args from the last `send(...)` call. Used by the no-tools retry UI (PR 3.B.2). */
    get lastSend() { return lastSend; },
    send,
    loadThread: (threadId: string) => loadThread(sdk, threadId),
    newSession,
    abort,
  };
  return api;
}
