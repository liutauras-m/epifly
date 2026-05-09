import type { ApiError, ApiResult } from './types';

export async function apiCall<T>(
  fetchFn: typeof fetch,
  url: string,
  init?: RequestInit
): Promise<ApiResult<T>> {
  try {
    const res = await fetchFn(url, {
      ...init,
      headers: { 'Content-Type': 'application/json', ...(init?.headers ?? {}) }
    });
    if (!res.ok) {
      let message = `HTTP ${res.status}`;
      try { const j = await res.json(); message = (j as { error?: string }).error ?? message; } catch {}
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
