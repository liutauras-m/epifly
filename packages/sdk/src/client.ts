import { auth } from './auth.js';
import { capabilities } from './capabilities.js';
import { chatApi } from './chatApi.js';
import { files } from './files.js';
import { threads } from './threads.js';
import { ui } from './ui.js';
import { workspaces } from './workspaces.js';
import { realtime } from './realtime.js';
import { shells } from './shells.js';
import type { ApiError, ApiResult } from './types.js';

export interface TokenProvider {
  get(): Promise<string | null>;
}

export interface ClientOpts {
  fetch: typeof globalThis.fetch;
  baseUrl: string;
  tokenProvider: TokenProvider;
}

export interface InternalClient {
  fetch: typeof globalThis.fetch;
  baseUrl: string;
  tokenProvider: TokenProvider;
  request<T>(method: string, path: string, body?: unknown, init?: RequestInit): Promise<T>;
  call<T>(method: string, path: string, body?: unknown, init?: RequestInit): Promise<ApiResult<T>>;
}

function createInternalClient(opts: ClientOpts): InternalClient {
  async function request<T>(
    method: string,
    path: string,
    body?: unknown,
    init?: RequestInit
  ): Promise<T> {
    const token = await opts.tokenProvider.get();
    const res = await opts.fetch(`${opts.baseUrl}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        ...init?.headers,
      },
      body: body != null ? JSON.stringify(body) : undefined,
      ...init,
    });
    if (!res.ok) throw new Error(`${method} ${path} → ${res.status} ${res.statusText}`);
    if (res.status === 204) return null as T;
    return res.json() as Promise<T>;
  }

  async function call<T>(
    method: string,
    path: string,
    body?: unknown,
    init?: RequestInit
  ): Promise<ApiResult<T>> {
    try {
      const token = await opts.tokenProvider.get();
      const res = await opts.fetch(`${opts.baseUrl}${path}`, {
        method,
        headers: {
          'Content-Type': 'application/json',
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
          ...init?.headers,
        },
        body: body != null ? JSON.stringify(body) : undefined,
        ...init,
      });
      if (!res.ok) {
        let message = `HTTP ${res.status}`;
        try {
          const j = await res.json() as { error?: unknown };
          if (typeof j.error === 'string') message = j.error;
          else if (j.error && typeof (j.error as Record<string, unknown>).message === 'string')
            message = (j.error as Record<string, unknown>).message as string;
        } catch {}
        return { data: null, error: { status: res.status, message } };
      }
      if (res.status === 204) return { data: null as unknown as T, error: null };
      const data = await res.json() as T;
      return { data, error: null };
    } catch (e: unknown) {
      const err: ApiError = { status: 0, message: e instanceof Error ? e.message : String(e) };
      return { data: null, error: err };
    }
  }

  return { ...opts, request, call };
}

export function createConusSdk(opts: ClientOpts) {
  const client = createInternalClient(opts);
  return {
    auth:         auth(client),
    capabilities: capabilities(client),
    chat:         chatApi(client),
    files:        files(client),
    threads:      threads(client),
    ui:           ui(client),
    workspaces:   workspaces(client),
    realtime:     realtime(client),
    shells:       shells(client),
  } as const;
}

export type ConusSdk = ReturnType<typeof createConusSdk>;

