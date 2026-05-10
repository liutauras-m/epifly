import type { ConusaiClient } from "./client.js";

export function auth(client: ConusaiClient) {
  return {
    async login(email: string, password: string): Promise<void> {
      await client.request("POST", "/api/auth/login", { email, password });
    },

    async logout(): Promise<void> {
      await client.request("POST", "/api/auth/logout");
    },
  };
}
