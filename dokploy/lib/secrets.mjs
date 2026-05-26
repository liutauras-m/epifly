/**
 * dokploy/lib/secrets.mjs
 * Secret generation helpers and the authoritative SECRETS / STATEFUL_SECRETS maps.
 * Zero dependencies (only node:crypto).
 */

import { randomBytes, generateKeyPairSync } from "node:crypto";

/**
 * Auto-generated secret recipes, keyed by Shared Env name.
 * Formats match generate-prod-env.mjs so downstream validation passes.
 * @type {Record<string, () => string>}
 */
export const SECRETS = {
  POSTGRES_PASSWORD:       () => randB64Url(30),    // ~40 chars
  ZITADEL_MASTERKEY:       () => randB64Url(24),    // exactly 32 chars
  LAGO_SECRET_KEY_BASE:    () => randHex(64),       // 128 hex chars
  LAGO_ENCRYPTION_DET_KEY: () => randB64Url(24),
  LAGO_ENCRYPTION_SALT:    () => randB64Url(24),
  LAGO_ENCRYPTION_KEY:     () => randB64Url(24),
  LAGO_RSA_PRIVATE_KEY:    () => base64(generateRsaPem(2048)),
  AWS_ACCESS_KEY_ID:       () => "rfs_" + randUpperAlnum(15),
  AWS_SECRET_ACCESS_KEY:   () => randB64Url(30),
  RUSTFS_IAM_ENC_KEY:      () => randB64Url(24),
  RUSTFS_WEBHOOK_SECRET:   () => randB64Url(32),
  UI_SESSION_KEY:          () => randHex(32),       // 64 hex chars (>32 bytes)
  PLATFORM_ADMIN_TOKEN:    () => "pat_" + randB64Url(32),
};

/**
 * Stateful secrets: once data is written to a volume with a given value,
 * regenerating that value silently corrupts the data.
 *
 * Map: SECRET_NAME -> volume-name that holds data encrypted/hashed by it.
 *
 * UI_SESSION_KEY and PLATFORM_ADMIN_TOKEN are intentionally NOT here —
 * they are runtime-only; rotation is safe (just invalidates sessions/tokens).
 *
 * @type {Record<string, string>}
 */
export const STATEFUL_SECRETS = {
  POSTGRES_PASSWORD:       "conusai_postgres_data",
  LAGO_SECRET_KEY_BASE:    "conusai_postgres_data",
  LAGO_ENCRYPTION_DET_KEY: "conusai_postgres_data",
  LAGO_ENCRYPTION_SALT:    "conusai_postgres_data",
  LAGO_ENCRYPTION_KEY:     "conusai_postgres_data",
  LAGO_RSA_PRIVATE_KEY:    "conusai_postgres_data",
  RUSTFS_IAM_ENC_KEY:      "conusai_rustfs_data",
  AWS_ACCESS_KEY_ID:       "conusai_rustfs_data",
  AWS_SECRET_ACCESS_KEY:   "conusai_rustfs_data",
  ZITADEL_MASTERKEY:       "conusai_postgres_data",
};

// ── Crypto helpers ───────────────────────────────────────────────────────────

/** @param {number} bytes */
export function randB64Url(bytes) {
  return randomBytes(bytes)
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/** @param {number} bytes - returns hex string of length bytes*2 */
export function randHex(bytes) {
  return randomBytes(bytes).toString("hex");
}

/** @param {number} n */
export function randUpperAlnum(n) {
  const charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  const buf = randomBytes(n);
  let s = "";
  for (let i = 0; i < n; i++) s += charset[buf[i] % charset.length];
  return s;
}

/** @param {string} s */
export function base64(s) {
  return Buffer.from(s, "utf8").toString("base64");
}

/** @param {number} modulusLength */
export function generateRsaPem(modulusLength) {
  return generateKeyPairSync("rsa", { modulusLength }).privateKey.export({
    type: "pkcs8",
    format: "pem",
  });
}
