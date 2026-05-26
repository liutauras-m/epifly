/**
 * E2E tests for `epifly verify` command.
 * Uses a mock HTTP server to simulate the endpoints being verified.
 */

import { createServer, Server } from "node:http";
import { AddressInfo } from "node:net";
import { buildVerifyChecks, runCheck } from "../../../../dokploy/lib/verify.mjs";

function startStaticServer(status: number, body: string, contentType = "text/plain"): Promise<Server> {
  return new Promise((resolve) => {
    const s = createServer((_, res) => {
      res.writeHead(status, { "content-type": contentType });
      res.end(body);
    });
    s.listen(0, "127.0.0.1", () => resolve(s));
  });
}

describe("runCheck", () => {
  test("returns true when status matches", async () => {
    const s = await startStaticServer(200, "ok");
    const port = (s.address() as AddressInfo).port;
    try {
      const result = await runCheck({
        url: `http://127.0.0.1:${port}/`,
        expectStatus: [200],
      });
      expect(result).toBe(true);
    } finally {
      await new Promise<void>((r) => s.close(() => r()));
    }
  });

  test("returns false when status does not match", async () => {
    const s = await startStaticServer(500, "error");
    const port = (s.address() as AddressInfo).port;
    try {
      const result = await runCheck({
        url: `http://127.0.0.1:${port}/`,
        expectStatus: [200, 301],
      });
      expect(result).toBe(false);
    } finally {
      await new Promise<void>((r) => s.close(() => r()));
    }
  });

  test("returns true when expectJsonKey is present in response", async () => {
    const s = await startStaticServer(
      200,
      JSON.stringify({ issuer: "https://auth.example.com" }),
      "application/json",
    );
    const port = (s.address() as AddressInfo).port;
    try {
      const result = await runCheck({
        url: `http://127.0.0.1:${port}/`,
        expectStatus: [200],
        expectJsonKey: "issuer",
      });
      expect(result).toBe(true);
    } finally {
      await new Promise<void>((r) => s.close(() => r()));
    }
  });

  test("returns false when expectJsonKey is missing from JSON response", async () => {
    const s = await startStaticServer(
      200,
      JSON.stringify({ other: "field" }),
      "application/json",
    );
    const port = (s.address() as AddressInfo).port;
    try {
      const result = await runCheck({
        url: `http://127.0.0.1:${port}/`,
        expectStatus: [200],
        expectJsonKey: "issuer",
      });
      expect(result).toBe(false);
    } finally {
      await new Promise<void>((r) => s.close(() => r()));
    }
  });

  test("returns false when non-JSON body and expectJsonKey set", async () => {
    const s = await startStaticServer(200, "not json");
    const port = (s.address() as AddressInfo).port;
    try {
      const result = await runCheck({
        url: `http://127.0.0.1:${port}/`,
        expectStatus: [200],
        expectJsonKey: "issuer",
      });
      expect(result).toBe(false);
    } finally {
      await new Promise<void>((r) => s.close(() => r()));
    }
  });
});

describe("buildVerifyChecks", () => {
  const checks = buildVerifyChecks("epifly.example.com");

  test("returns an array of checks", () => {
    expect(Array.isArray(checks)).toBe(true);
    expect(checks.length).toBeGreaterThan(4);
  });

  test("includes zitadel OIDC check with expectJsonKey=issuer", () => {
    const oidc = checks.find((c) => c.url.includes(".well-known/openid-configuration"));
    expect(oidc).toBeDefined();
    expect(oidc?.expectJsonKey).toBe("issuer");
  });

  test("all checks have url, expectStatus array", () => {
    for (const c of checks) {
      expect(typeof c.url).toBe("string");
      expect(Array.isArray(c.expectStatus)).toBe(true);
      expect(c.expectStatus.length).toBeGreaterThan(0);
    }
  });

  test("all URLs use the provided domain", () => {
    for (const c of checks) {
      expect(c.url).toContain("epifly.example.com");
    }
  });
});
