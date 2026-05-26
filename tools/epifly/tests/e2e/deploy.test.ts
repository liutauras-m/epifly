/**
 * E2E tests for the deploy command — trigger + status polling.
 * Uses MockDokployServer to simulate the Dokploy API.
 */

import { makeClient } from "../../../../dokploy/lib/dokploy-client.mjs";
import { MockDokployServer } from "./mock-server.ts";

async function simulateDeploy(
  api: ReturnType<typeof makeClient>,
  environmentId: string,
): Promise<string> {
  // Locate epifly-deploy compose
  const search: any = await api.query("compose.search", {
    environmentId,
    limit: 100,
    offset: 0,
  });
  const self = (search?.items ?? []).find((c: any) => c.name === "epifly-deploy");
  if (!self) throw new Error("epifly-deploy not found");

  // Trigger deploy
  await api.mutate("compose.deploy", { composeId: self.composeId });

  // Poll status (the mock immediately returns 'done')
  const compose: any = await api.query("compose.one", { composeId: self.composeId });
  return compose.composeStatus;
}

describe("deploy flow", () => {
  let server: MockDokployServer;
  let api: ReturnType<typeof makeClient>;

  beforeAll(async () => {
    server = new MockDokployServer();
    await server.start();
    api = makeClient(server.baseUrl, "test-key");
  });

  afterAll(async () => await server.stop());

  test("locates epifly-deploy compose", async () => {
    const search: any = await api.query("compose.search", {
      environmentId: "env-1",
      limit: 100,
      offset: 0,
    });
    const self = (search?.items ?? []).find((c: any) => c.name === "epifly-deploy");
    expect(self).toBeDefined();
    expect(self.composeId).toBe("compose-orchestrator");
  });

  test("deploy returns done status", async () => {
    const status = await simulateDeploy(api, "env-1");
    expect(status).toBe("done");
  });

  test("compose.deploy call is recorded", async () => {
    server.calls.length = 0; // clear
    await simulateDeploy(api, "env-1");
    const deployCalls = server.calls.filter(([p]) => p === "compose.deploy");
    expect(deployCalls.length).toBeGreaterThanOrEqual(1);
  });

  test("compose status changes to done after deploy", async () => {
    // Reset to idle
    const c = server.composes.get("compose-orchestrator");
    if (c) c.composeStatus = "idle";

    await api.mutate("compose.deploy", { composeId: "compose-orchestrator" });
    const updated: any = await api.query("compose.one", { composeId: "compose-orchestrator" });
    expect(updated.composeStatus).toBe("done");
  });

  test("deploy fails gracefully for non-existent compose", async () => {
    // The mock returns null for non-existent compose; the client should either
    // return null or throw — not crash the process.
    let caughtNull = false;
    try {
      const result = await api.mutate("compose.deploy", { composeId: "non-existent-id" });
      if (result === null) caughtNull = true;
    } catch {
      caughtNull = true; // either outcome is acceptable
    }
    expect(caughtNull).toBe(true);
  });
});

describe("orchestrator phase env passthrough", () => {
  let server: MockDokployServer;
  let api: ReturnType<typeof makeClient>;

  beforeAll(async () => {
    server = new MockDokployServer();
    await server.start();
    api = makeClient(server.baseUrl, "test-key");
  });

  afterAll(async () => await server.stop());

  test("project.update stores env text and project.one retrieves it", async () => {
    const envText = "APP_DOMAIN=test.example.com\nDOKPLOY_URL=https://dokploy.example.com\n";
    await api.mutate("project.update", { projectId: "proj-1", env: envText });
    const project: any = await api.query("project.one", { projectId: "proj-1" });
    expect(project.env).toBe(envText);
  });

  test("compose.update stores composePath", async () => {
    await api.mutate("compose.update", {
      composeId: "compose-orchestrator",
      composePath: "./dokploy/infra/docker-compose.yml",
    });
    const c: any = await api.query("compose.one", { composeId: "compose-orchestrator" });
    expect(c.composePath).toBe("./dokploy/infra/docker-compose.yml");
  });
});
