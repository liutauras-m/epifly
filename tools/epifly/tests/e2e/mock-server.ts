/**
 * Minimal mock Dokploy REST server for e2e tests.
 *
 * Mimics only the procedures used by epifly commands:
 *   - project.all, project.one, project.update
 *   - environment.one, environment.search
 *   - compose.search, compose.one, compose.create, compose.update, compose.deploy
 *
 * Usage:
 *   const server = new MockDokployServer();
 *   await server.start();
 *   // ... tests ...
 *   await server.stop();
 */

import { createServer, IncomingMessage, ServerResponse, Server } from "node:http";
import { AddressInfo } from "node:net";

export interface MockCompose {
  composeId: string;
  name: string;
  composeStatus: string;
  environmentId?: string;
  serverId?: string;
  composePath?: string;
  env?: string;
  sourceType?: string;
  owner?: string;
  repository?: string;
  branch?: string;
}

export interface MockProject {
  projectId: string;
  name: string;
  env: string;
}

export interface MockEnvironment {
  environmentId: string;
  name: string;
  projectId: string;
}

export class MockDokployServer {
  private server: Server;
  private _port = 0;

  public projects: Map<string, MockProject> = new Map([
    ["proj-1", { projectId: "proj-1", name: "Epifly", env: "" }],
  ]);

  public environments: Map<string, MockEnvironment> = new Map([
    [
      "env-1",
      { environmentId: "env-1", name: "production", projectId: "proj-1" },
    ],
  ]);

  public composes: Map<string, MockCompose> = new Map([
    [
      "compose-orchestrator",
      {
        composeId: "compose-orchestrator",
        name: "epifly-deploy",
        composeStatus: "done",
        sourceType: "github",
        owner: "conusai",
        repository: "conusai-platform",
        branch: "main",
        env: "",
      },
    ],
  ]);

  /** Calls received: [[procedure, input], ...] */
  public calls: Array<[string, unknown]> = [];

  constructor() {
    this.server = createServer((req, res) => this.handle(req, res));
  }

  async start(): Promise<void> {
    return new Promise((resolve) => {
      this.server.listen(0, "127.0.0.1", () => {
        this._port = (this.server.address() as AddressInfo).port;
        resolve();
      });
    });
  }

  async stop(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.server.close((err) => (err ? reject(err) : resolve()));
    });
  }

  get port(): number { return this._port; }
  get baseUrl(): string { return `http://127.0.0.1:${this._port}`; }

  private async handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const url = new URL(req.url!, `http://127.0.0.1`);
    const path = url.pathname; // /api/<procedure>
    const procedure = path.replace(/^\/api\//, "");

    let input: any = {};
    if (req.method === "GET") {
      for (const [k, v] of url.searchParams.entries()) {
        input[k] = v;
      }
    } else if (req.method === "POST") {
      const body = await readBody(req);
      try { input = body ? JSON.parse(body) : {}; } catch {}
    }

    this.calls.push([procedure, input]);

    let data: unknown;
    try {
      data = this.dispatch(procedure, input);
    } catch (err) {
      res.writeHead(500, { "content-type": "application/json" });
      res.end(JSON.stringify({ message: String(err) }));
      return;
    }
    if (data === null) {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ message: `Unknown procedure or resource: ${procedure}` }));
      return;
    }

    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(data));
  }

  private dispatch(procedure: string, input: any): unknown {
    switch (procedure) {
      // ── project ────────────────────────────────────────────────────────────
      case "project.all":
        return [...this.projects.values()];

      case "project.one": {
        const p = this.projects.get(input.projectId);
        if (!p) return null;
        return p;
      }

      case "project.update": {
        const p = this.projects.get(input.projectId);
        if (!p) return null;
        if (input.env !== undefined) p.env = input.env;
        return p;
      }

      // ── environment ────────────────────────────────────────────────────────
      case "environment.search":
        return { items: [...this.environments.values()] };

      case "environment.one": {
        const e = this.environments.get(input.environmentId);
        if (!e) return null;
        return e;
      }

      // ── compose ────────────────────────────────────────────────────────────
      case "compose.search": {
        const items = [...this.composes.values()].filter(
          (c) => !input.environmentId || !c.environmentId || c.environmentId === input.environmentId,
        );
        return { items, total: items.length };
      }

      case "compose.one": {
        const c = this.composes.get(input.composeId);
        if (!c) return null;
        return c;
      }

      case "compose.create": {
        const id = `compose-${Date.now()}`;
        const c: MockCompose = {
          composeId: id,
          name: input.name,
          composeStatus: "idle",
          env: "",
          ...(input.environmentId && { environmentId: input.environmentId }),
          ...(input.serverId && { serverId: input.serverId }),
          ...(input.composePath && { composePath: input.composePath }),
          ...(input.sourceType && { sourceType: input.sourceType }),
        };
        this.composes.set(id, c);
        return c;
      }

      case "compose.update": {
        const c = this.composes.get(input.composeId);
        if (!c) return null;
        Object.assign(c, input);
        return c;
      }

      case "compose.deploy": {
        const c = this.composes.get(input.composeId);
        if (!c) return null;
        c.composeStatus = "done"; // immediately done for testing
        return { ok: true };
      }

      default:
        return null;
    }
  }
}

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}
