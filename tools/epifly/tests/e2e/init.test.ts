/**
 * E2E tests for the init wizard logic.
 * Tests config loading/writing without spawning interactive prompts.
 */

import { writeFileSync, readFileSync, unlinkSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { makeClient } from "../../../../dokploy/lib/dokploy-client.mjs";
import { MockDokployServer } from "./mock-server.ts";

const CONFIG_ENV_KEYS = [
  "DOKPLOY_URL",
  "DOKPLOY_API_KEY",
  "DOKPLOY_ENVIRONMENT_ID",
  "APP_DOMAIN",
  "EPIFLY_CONFIG",
] as const;

const SAVED_ENV: Record<string, string | undefined> = {};

beforeAll(() => {
  for (const key of CONFIG_ENV_KEYS) {
    SAVED_ENV[key] = process.env[key];
  }
});

beforeEach(() => {
  for (const key of CONFIG_ENV_KEYS) {
    delete process.env[key];
  }
});

afterAll(() => {
  for (const key of CONFIG_ENV_KEYS) {
    const value = SAVED_ENV[key];
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

function writeTmpConfig(content: object): string {
  const path = join(tmpdir(), `.dokploy-test-${Date.now()}`);
  writeFileSync(path, JSON.stringify(content), "utf8");
  return path;
}

describe("config loading", () => {
  test("reads all required fields from a JSON config file", async () => {
    const { loadConfig } = await import("../../src/lib/config.ts");
    const path = writeTmpConfig({
      dokployUrl: "https://dokploy.example.com",
      apiKey: "test-key",
      environmentId: "env-1",
      appDomain: "epifly.example.com",
    });
    try {
      const cfg = loadConfig({ config: path });
      expect(cfg.dokployUrl).toBe("https://dokploy.example.com");
      expect(cfg.apiKey).toBe("test-key");
      expect(cfg.environmentId).toBe("env-1");
      expect(cfg.appDomain).toBe("epifly.example.com");
    } finally {
      unlinkSync(path);
    }
  });

  test("strips trailing slash from dokplayUrl", async () => {
    const { loadConfig } = await import("../../src/lib/config.ts");
    const path = writeTmpConfig({
      dokployUrl: "https://dokploy.example.com///",
      apiKey: "test-key",
      environmentId: "env-1",
      appDomain: "epifly.example.com",
    });
    try {
      const cfg = loadConfig({ config: path });
      expect(cfg.dokployUrl).toBe("https://dokploy.example.com");
    } finally {
      unlinkSync(path);
    }
  });

  test("throws if required fields are missing", async () => {
    const { loadConfig } = await import("../../src/lib/config.ts");
    const path = writeTmpConfig({ dokployUrl: "https://dokploy.example.com" });
    try {
      expect(() => loadConfig({ config: path })).toThrow(/apiKey|environmentId|appDomain/i);
    } finally {
      unlinkSync(path);
    }
  });

  test("env vars override config file values", async () => {
    const { loadConfig } = await import("../../src/lib/config.ts");
    const path = writeTmpConfig({
      dokployUrl: "https://dokploy.example.com",
      apiKey: "file-key",
      environmentId: "env-from-file",
      appDomain: "epifly.example.com",
    });
    const savedEnv = process.env["DOKPLOY_API_KEY"];
    process.env["DOKPLOY_API_KEY"] = "env-key";
    try {
      const cfg = loadConfig({ config: path });
      expect(cfg.apiKey).toBe("env-key");
    } finally {
      if (savedEnv === undefined) delete process.env["DOKPLOY_API_KEY"];
      else process.env["DOKPLOY_API_KEY"] = savedEnv;
      unlinkSync(path);
    }
  });
});

describe("init: Dokploy API connectivity", () => {
  let server: MockDokployServer;

  beforeAll(async () => {
    server = new MockDokployServer();
    await server.start();
  });

  afterAll(async () => await server.stop());

  test("environment.search returns available environments", async () => {
    const api = makeClient(server.baseUrl, "test-key");
    const result: any = await api.query("environment.search", {});
    expect(result.items.length).toBeGreaterThan(0);
    expect(result.items[0]).toHaveProperty("environmentId");
    expect(result.items[0]).toHaveProperty("name");
  });

  test("can verify connectivity by calling project.all", async () => {
    const api = makeClient(server.baseUrl, "test-key");
    await expect(api.query("project.all", {})).resolves.toBeDefined();
  });

  test("throws on invalid API key (404 procedure)", async () => {
    const api = makeClient(server.baseUrl, "bad-key");
    await expect(api.query("nonexistent.procedure", {})).rejects.toThrow();
  });
});
