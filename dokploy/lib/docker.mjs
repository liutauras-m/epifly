/**
 * dokploy/lib/docker.mjs
 * Docker Engine API helpers (via Unix socket).
 * Used only by the orchestrator (deploy.mjs) — not by the CLI.
 * Zero dependencies (uses node:http).
 */

import { request as httpRequest } from "node:http";
import { existsSync } from "node:fs";
import { DOCKER_SOCK } from "./manifest.mjs";

/**
 * Make a raw request against the Docker Engine API over the Unix socket.
 *
 * @param {"GET"|"POST"|"DELETE"} method
 * @param {string} path   e.g. "/volumes/create"
 * @param {object} [body] JSON body for POST requests
 * @returns {Promise<{status: number, body: string}>}
 */
export function dockerApi(method, path, body) {
  return new Promise((resolve, reject) => {
    const req = httpRequest(
      {
        socketPath: DOCKER_SOCK,
        method,
        path,
        headers: body
          ? { "content-type": "application/json", accept: "application/json" }
          : { accept: "application/json" },
      },
      (r) => {
        const chunks = [];
        r.on("data", (c) => chunks.push(c));
        r.on("end", () =>
          resolve({ status: r.statusCode, body: Buffer.concat(chunks).toString("utf8") }),
        );
      },
    );
    req.on("error", reject);
    if (body) req.write(JSON.stringify(body));
    req.end();
  });
}

/**
 * Returns the subset of `names` that currently exist as named Docker volumes.
 * Silently returns an empty set when the Docker socket is not mounted.
 *
 * @param {Set<string>} names  Volume names to check.
 * @returns {Promise<Set<string>>}
 */
export async function listExistingVolumes(names) {
  const out = new Set();
  if (!existsSync(DOCKER_SOCK)) return out;
  for (const name of names) {
    const r = await dockerApi("GET", `/volumes/${encodeURIComponent(name)}`);
    if (r.status === 200) {
      out.add(name);
    } else if (r.status !== 404) {
      throw new Error(
        `docker GET /volumes/${name} → HTTP ${r.status}: ${r.body.slice(0, 200)}`,
      );
    }
  }
  return out;
}
