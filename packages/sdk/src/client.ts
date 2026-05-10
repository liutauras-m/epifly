export interface ClientOptions {
  baseUrl: string;
  tokenProvider: () => Promise<string>;
}

export function createClient(options: ClientOptions) {
  const { baseUrl, tokenProvider } = options;

  async function request<T>(
    method: string,
    path: string,
    body?: unknown,
    init?: RequestInit
  ): Promise<T> {
    const token = await tokenProvider();
    const res = await fetch(`${baseUrl}${path}`, {
      method,
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
        ...init?.headers,
      },
      body: body != null ? JSON.stringify(body) : undefined,
      ...init,
    });
    if (!res.ok) {
      throw new Error(`${method} ${path} → ${res.status} ${res.statusText}`);
    }
    return res.json() as Promise<T>;
  }

  return { request, baseUrl, tokenProvider };
}

export type ConusaiClient = ReturnType<typeof createClient>;
