import type { FileToken } from "@conusai/types";
import type { ConusaiClient } from "./client.js";

export function files(client: ConusaiClient) {
  return {
    async upload(file: File): Promise<FileToken> {
      const token = await client.tokenProvider();
      const form = new FormData();
      form.append("file", file);
      const res = await fetch(`${client.baseUrl}/v1/files`, {
        method: "POST",
        headers: { Authorization: `Bearer ${token}` },
        body: form,
      });
      if (!res.ok) {
        throw new Error(`upload → ${res.status} ${res.statusText}`);
      }
      return res.json() as Promise<FileToken>;
    },
  };
}
