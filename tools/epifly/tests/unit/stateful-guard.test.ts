/**
 * Unit tests — stateful secret guard logic.
 *
 * Validates the invariant: if a volume bound to a stateful secret already
 * exists, the orchestrator/CLI must NOT regenerate that secret.
 *
 * We test this as pure logic by mimicking what deploy.mjs's ensureSharedEnv
 * does: checking whether a key is in STATEFUL_SECRETS and whether its bound
 * volume is in the `existingVolumes` set.
 */

import { SECRETS, STATEFUL_SECRETS } from "../../../../dokploy/lib/secrets.mjs";

/**
 * Simulates the guard logic from deploy.mjs ensureSharedEnv:
 * Returns 'blocked' if regeneration would be refused, 'allowed' otherwise.
 */
function checkGuard(
  key: string,
  existingVolumes: Set<string>,
  currentValue: string,
): "blocked" | "allowed" | "no-generator" {
  if (!(key in SECRETS)) return "no-generator";
  if (currentValue.trim()) return "allowed"; // already has a value — guard is irrelevant
  const boundVolume = (STATEFUL_SECRETS as Record<string, string>)[key];
  if (boundVolume && existingVolumes.has(boundVolume)) return "blocked";
  return "allowed";
}

describe("stateful secret guard", () => {
  // ── Keys that are in STATEFUL_SECRETS ──────────────────────────────────────
  const statefulKeys = Object.keys(STATEFUL_SECRETS) as Array<keyof typeof STATEFUL_SECRETS>;

  test.each(statefulKeys)(
    "blocks regeneration of %s when bound volume exists",
    (key) => {
      const boundVolume = STATEFUL_SECRETS[key];
      const existingVolumes = new Set([boundVolume]);
      expect(checkGuard(key, existingVolumes, "")).toBe("blocked");
    },
  );

  test.each(statefulKeys)(
    "allows regeneration of %s on a fresh host (no volumes)",
    (key) => {
      const existingVolumes = new Set<string>();
      expect(checkGuard(key, existingVolumes, "")).toBe("allowed");
    },
  );

  test.each(statefulKeys)(
    "allows preserving existing value of %s even when volume exists",
    (key) => {
      const boundVolume = STATEFUL_SECRETS[key];
      const existingVolumes = new Set([boundVolume]);
      expect(checkGuard(key, existingVolumes, "already-set-value")).toBe("allowed");
    },
  );

  // ── Rotation-safe keys (NOT in STATEFUL_SECRETS) ──────────────────────────
  test("allows UI_SESSION_KEY regeneration even when postgres volume exists", () => {
    const existingVolumes = new Set(["conusai_postgres_data"]);
    expect(checkGuard("UI_SESSION_KEY", existingVolumes, "")).toBe("allowed");
  });

  test("allows PLATFORM_ADMIN_TOKEN regeneration even when postgres volume exists", () => {
    const existingVolumes = new Set(["conusai_postgres_data"]);
    expect(checkGuard("PLATFORM_ADMIN_TOKEN", existingVolumes, "")).toBe("allowed");
  });

  test("returns no-generator for unknown key", () => {
    const existingVolumes = new Set(["conusai_postgres_data"]);
    expect(checkGuard("TOTALLY_UNKNOWN_KEY", existingVolumes, "")).toBe("no-generator");
  });

  // ── Guard does not block when a DIFFERENT volume exists ───────────────────
  test("POSTGRES_PASSWORD: only blocked when postgres volume exists, not redis", () => {
    const onlyRedis = new Set(["conusai_redis_data"]);
    expect(checkGuard("POSTGRES_PASSWORD", onlyRedis, "")).toBe("allowed");
  });

  test("RUSTFS_IAM_ENC_KEY: only blocked when rustfs volume exists, not postgres", () => {
    const onlyPostgres = new Set(["conusai_postgres_data"]);
    expect(checkGuard("RUSTFS_IAM_ENC_KEY", onlyPostgres, "")).toBe("allowed");
  });
});
