/**
 * Unit tests for dokploy/lib/secrets.mjs
 * Covers: randB64Url, randHex, randUpperAlnum, base64, SECRETS generators,
 * and STATEFUL_SECRETS completeness.
 */

import {
  randB64Url,
  randHex,
  randUpperAlnum,
  base64,
  generateRsaPem,
  SECRETS,
  STATEFUL_SECRETS,
} from "../../../../dokploy/lib/secrets.mjs";

describe("randB64Url", () => {
  test("returns a string of the expected approximate length", () => {
    // base64url of N bytes ≈ ceil(N * 4/3) chars (no padding)
    const s = randB64Url(24);
    expect(s.length).toBeGreaterThan(20);
    expect(s.length).toBeLessThan(50);
  });

  test("output contains only url-safe base64 chars", () => {
    const s = randB64Url(64);
    expect(s).toMatch(/^[A-Za-z0-9\-_]+$/);
  });

  test("no = padding", () => {
    expect(randB64Url(30)).not.toContain("=");
  });

  test("each call produces a different value", () => {
    expect(randB64Url(32)).not.toBe(randB64Url(32));
  });
});

describe("randHex", () => {
  test("returns hex string of length bytes*2", () => {
    expect(randHex(32)).toHaveLength(64);
    expect(randHex(16)).toHaveLength(32);
  });

  test("only hex chars", () => {
    expect(randHex(32)).toMatch(/^[0-9a-f]+$/);
  });
});

describe("randUpperAlnum", () => {
  test("returns string of requested length", () => {
    expect(randUpperAlnum(15)).toHaveLength(15);
  });

  test("only uppercase alphanumeric chars", () => {
    expect(randUpperAlnum(30)).toMatch(/^[A-Z0-9]+$/);
  });
});

describe("base64", () => {
  test("encodes a string to valid base64", () => {
    const encoded = base64("hello world");
    expect(Buffer.from(encoded, "base64").toString("utf8")).toBe("hello world");
  });
});

describe("generateRsaPem", () => {
  test("returns a valid PKCS8 PEM", () => {
    const pem = generateRsaPem(2048);
    expect(pem).toContain("-----BEGIN PRIVATE KEY-----");
    expect(pem).toContain("-----END PRIVATE KEY-----");
  });
});

describe("SECRETS", () => {
  const knownKeys = [
    "POSTGRES_PASSWORD",
    "ZITADEL_MASTERKEY",
    "LAGO_SECRET_KEY_BASE",
    "LAGO_ENCRYPTION_DET_KEY",
    "LAGO_ENCRYPTION_SALT",
    "LAGO_ENCRYPTION_KEY",
    "LAGO_RSA_PRIVATE_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "RUSTFS_IAM_ENC_KEY",
    "RUSTFS_WEBHOOK_SECRET",
    "UI_SESSION_KEY",
    "PLATFORM_ADMIN_TOKEN",
  ];

  test.each(knownKeys)("SECRETS.%s() returns a non-empty string", (key) => {
    const value = SECRETS[key]();
    expect(typeof value).toBe("string");
    expect(value.length).toBeGreaterThan(8);
  });

  test("AWS_ACCESS_KEY_ID starts with rfs_", () => {
    expect(SECRETS.AWS_ACCESS_KEY_ID()).toMatch(/^rfs_/);
  });

  test("PLATFORM_ADMIN_TOKEN starts with pat_", () => {
    expect(SECRETS.PLATFORM_ADMIN_TOKEN()).toMatch(/^pat_/);
  });

  test("LAGO_RSA_PRIVATE_KEY is valid base64-encoded PKCS8", () => {
    const b64 = SECRETS.LAGO_RSA_PRIVATE_KEY();
    const pem = Buffer.from(b64, "base64").toString("utf8");
    expect(pem).toContain("-----BEGIN PRIVATE KEY-----");
  });
});

describe("STATEFUL_SECRETS", () => {
  test("all stateful keys are present in SECRETS", () => {
    for (const key of Object.keys(STATEFUL_SECRETS)) {
      expect(SECRETS).toHaveProperty(key);
    }
  });

  test("all volumes referenced are named correctly", () => {
    for (const vol of Object.values(STATEFUL_SECRETS)) {
      expect(vol).toMatch(/^conusai_/);
    }
  });

  test("UI_SESSION_KEY is NOT stateful (rotation-safe)", () => {
    expect(STATEFUL_SECRETS).not.toHaveProperty("UI_SESSION_KEY");
  });

  test("PLATFORM_ADMIN_TOKEN is NOT stateful (rotation-safe)", () => {
    expect(STATEFUL_SECRETS).not.toHaveProperty("PLATFORM_ADMIN_TOKEN");
  });
});
