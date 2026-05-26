/**
 * E2E tests for the Dokploy REST client (dokploy-client.mjs) against MockDokployServer.
 */

import { makeClient } from "../../../../dokploy/lib/dokploy-client.mjs";
import { MockDokployServer } from "./mock-server.ts";

describe("makeClient against mock server", () => {
  let server: MockDokployServer;
  let api: ReturnType<typeof makeClient>;

  beforeAll(async () => {
    server = new MockDokployServer();
    await server.start();
    api = makeClient(server.baseUrl, "test-api-key");
  });

  afterAll(async () => {
    await server.stop();
  });

  test("query: project.all returns projects", async () => {
    const result: any = await api.query("project.all", {});
    expect(Array.isArray(result)).toBe(true);
    expect(result).toHaveLength(1);
    expect(result[0].projectId).toBe("proj-1");
  });

  test("query: project.one returns project by id", async () => {
    const result: any = await api.query("project.one", { projectId: "proj-1" });
    expect(result.name).toBe("Epifly");
  });

  test("query: environment.search returns environments", async () => {
    const result: any = await api.query("environment.search", {});
    expect(result.items).toHaveLength(1);
    expect(result.items[0].environmentId).toBe("env-1");
  });

  test("query: environment.one returns environment", async () => {
    const result: any = await api.query("environment.one", { environmentId: "env-1" });
    expect(result.projectId).toBe("proj-1");
  });

  test("query: compose.search returns composes", async () => {
    const result: any = await api.query("compose.search", { environmentId: "env-1", limit: 100, offset: 0 });
    expect(result.items).toHaveLength(1);
    expect(result.items[0].name).toBe("epifly-deploy");
  });

  test("mutate: compose.deploy marks compose as done", async () => {
    await api.mutate("compose.deploy", { composeId: "compose-orchestrator" });
    const compose: any = await api.query("compose.one", { composeId: "compose-orchestrator" });
    expect(compose.composeStatus).toBe("done");
  });

  test("mutate: project.update stores env text", async () => {
    await api.mutate("project.update", { projectId: "proj-1", env: "FOO=bar\nBAZ=qux\n" });
    const project: any = await api.query("project.one", { projectId: "proj-1" });
    expect(project.env).toBe("FOO=bar\nBAZ=qux\n");
  });

  test("records all calls made", async () => {
    expect(server.calls.length).toBeGreaterThan(0);
    const procedures = server.calls.map(([p]) => p);
    expect(procedures).toContain("project.all");
    expect(procedures).toContain("compose.search");
  });

  test("throws on 404 procedure", async () => {
    await expect(api.query("nonexistent.procedure", {})).rejects.toThrow();
  });
});
