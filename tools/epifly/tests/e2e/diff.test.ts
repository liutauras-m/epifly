/**
 * E2E tests for the diff command logic.
 * Tests the dotenv comparison logic without spawning a subprocess.
 */

import { writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseDotenv, isSecret } from "../../../../dokploy/lib/dotenv.mjs";

function diffEnvs(
  local: Record<string, string>,
  remote: Record<string, string>,
): {
  remoteOnly: string[];
  localOnly: string[];
  changed: string[];
  same: string[];
} {
  const allKeys = new Set([...Object.keys(local), ...Object.keys(remote)]);
  const remoteOnly: string[] = [];
  const localOnly: string[] = [];
  const changed: string[] = [];
  const same: string[] = [];

  for (const k of allKeys) {
    if (!(k in local) && k in remote) remoteOnly.push(k);
    else if (k in local && !(k in remote)) localOnly.push(k);
    else if (local[k] !== remote[k]) changed.push(k);
    else same.push(k);
  }

  return { remoteOnly, localOnly, changed, same };
}

describe("diff env logic", () => {
  test("detects keys only in remote", () => {
    const { remoteOnly } = diffEnvs({ A: "1" }, { A: "1", B: "2" });
    expect(remoteOnly).toContain("B");
  });

  test("detects keys only in local", () => {
    const { localOnly } = diffEnvs({ A: "1", C: "3" }, { A: "1" });
    expect(localOnly).toContain("C");
  });

  test("detects changed values", () => {
    const { changed } = diffEnvs({ A: "old" }, { A: "new" });
    expect(changed).toContain("A");
  });

  test("marks unchanged keys as same", () => {
    const { same } = diffEnvs({ A: "1", B: "2" }, { A: "1", B: "2" });
    expect(same).toContain("A");
    expect(same).toContain("B");
  });

  test("identical envs produce all-same diff", () => {
    const env = { FOO: "bar", BAZ: "qux" };
    const { remoteOnly, localOnly, changed } = diffEnvs(env, { ...env });
    expect(remoteOnly).toHaveLength(0);
    expect(localOnly).toHaveLength(0);
    expect(changed).toHaveLength(0);
  });

  test("empty vs populated produces all remote-only", () => {
    const { remoteOnly } = diffEnvs({}, { A: "1", B: "2" });
    expect(remoteOnly).toHaveLength(2);
  });
});

describe("secret masking in diff", () => {
  test("password key identified as secret", () => {
    expect(isSecret("POSTGRES_PASSWORD")).toBe(true);
  });

  test("non-secret key not masked", () => {
    expect(isSecret("APP_DOMAIN")).toBe(false);
    expect(isSecret("POSTGRES_USER")).toBe(false);
  });
});

describe("parseDotenv integration with diff", () => {
  test("parses a realistic .env.production file", () => {
    const content = `
# Production environment
APP_DOMAIN=epifly.example.com
DOKPLOY_URL=https://dokploy.example.com
POSTGRES_PASSWORD=super_secret_123
OPENAI_API_KEY=sk-test-key
`;
    const parsed = parseDotenv(content);
    expect(parsed["APP_DOMAIN"]).toBe("epifly.example.com");
    expect(parsed["POSTGRES_PASSWORD"]).toBe("super_secret_123");
    expect(parsed["OPENAI_API_KEY"]).toBe("sk-test-key");
    expect(Object.keys(parsed)).not.toContain(""); // no empty keys
  });
});
