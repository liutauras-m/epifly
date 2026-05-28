#!/usr/bin/env node
/**
 * scripts/zitadel-assert-token-shape.mjs — Z.10
 *
 * Provisions a throwaway test user via the Zitadel Management API, mints an
 * access token via the OIDC password grant (service accounts only), decodes it,
 * and asserts that the exact claim keys the gateway expects are present.
 *
 * Writes tests/fixtures/zitadel-token-shape.json for downstream test suites.
 *
 * Required env vars:
 *   ZITADEL_ISSUER
 *   ZITADEL_BOOTSTRAP_PAT          IAM_OWNER service-user PAT
 *   ZITADEL_GATEWAY_CLIENT_ID      from bootstrap
 *   ZITADEL_GATEWAY_INTROSPECT_SECRET
 *   ZITADEL_WEB_CLIENT_ID          from bootstrap
 *
 * Optional (for custom claim mapping):
 *   ZITADEL_ORG_CLAIM              default: urn:zitadel:iam:user:resourceowner:id
 *   ZITADEL_ROLES_CLAIM            default: urn:zitadel:iam:org:project:roles
 */

import { writeFileSync, mkdirSync } from "node:fs";
import { createHash, randomUUID } from "node:crypto";

const issuer = requireEnv("ZITADEL_ISSUER");
const pat = requireEnv("ZITADEL_BOOTSTRAP_PAT");
const gatewayClientId = requireEnv("ZITADEL_GATEWAY_CLIENT_ID");
const gatewaySecret = requireEnv("ZITADEL_GATEWAY_INTROSPECT_SECRET");
const webClientId = requireEnv("ZITADEL_WEB_CLIENT_ID");
const orgClaim = process.env.ZITADEL_ORG_CLAIM ?? "urn:zitadel:iam:user:resourceowner:id";
const rolesClaim = process.env.ZITADEL_ROLES_CLAIM ?? "urn:zitadel:iam:org:project:roles";

const mgmt = `${issuer}/management/v1`;
const mgmtHeaders = {
  Authorization: `Bearer ${pat}`,
  "Content-Type": "application/json",
};

function requireEnv(name) {
  const v = process.env[name];
  if (!v) {
    console.error(`Missing required env var: ${name}`);
    process.exit(1);
  }
  return v;
}

async function mgmtApi(method, path, body) {
  const res = await fetch(`${mgmt}${path}`, {
    method,
    headers: mgmtHeaders,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  if (!res.ok) {
    throw new Error(`${method} ${path} → HTTP ${res.status}: ${text}`);
  }
  return text ? JSON.parse(text) : {};
}

function decodeJwtPayload(token) {
  const parts = token.split(".");
  if (parts.length < 2) throw new Error("invalid JWT");
  const pad = (s) => s + "=".repeat((4 - (s.length % 4)) % 4);
  const json = Buffer.from(pad(parts[1]), "base64url").toString("utf8");
  return JSON.parse(json);
}

async function main() {
  console.log(`\nZitadel token-shape assertion — issuer: ${issuer}\n`);

  // 1. Create throwaway test user
  const suffix = Date.now();
  const testEmail = `shape-test-${suffix}@assert.test.epifly`;
  const testPassword = `AssertPw${randomUUID().replace(/-/g, "")}!1Aa`;

  console.log("1. Creating throwaway test user…");
  let userId;
  try {
    const res = await mgmtApi("POST", "/users/human/_import", {
      userName: testEmail,
      profile: { firstName: "TokenShape", lastName: "Assert" },
      email: { email: testEmail, isEmailVerified: true },
      password: { value: testPassword, changeRequired: false },
    });
    userId = res.userId;
    console.log(`   Created user: ${userId}`);
  } catch (e) {
    throw new Error(`User creation failed: ${e.message}`);
  }

  // 2. Mint an access token via ROPC (resource-owner password credentials)
  //    NOTE: ROPC is only supported for service users / testing in Zitadel when
  //    explicitly enabled. In production this grant type should be disabled.
  //    If unavailable, we fall back to introspection of the bootstrap PAT.
  console.log("2. Minting access token…");
  let accessToken;

  try {
    // Try ROPC grant
    const tokenRes = await fetch(`${issuer}/oauth/v2/token`, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        grant_type: "password",
        username: testEmail,
        password: testPassword,
        client_id: webClientId,
        scope: `openid profile email urn:zitadel:iam:org:project:id:${gatewayClientId}:aud`,
      }),
    });
    if (!tokenRes.ok) {
      const txt = await tokenRes.text();
      throw new Error(`ROPC grant failed: HTTP ${tokenRes.status} — ${txt}`);
    }
    const tokenData = await tokenRes.json();
    accessToken = tokenData.access_token;
    console.log("   ROPC token obtained.");
  } catch (ropcErr) {
    console.warn(`   ROPC unavailable (${ropcErr.message}); falling back to bootstrap PAT introspection.`);
    // Bootstrap PAT is a JWT in some Zitadel setups; try to decode it.
    try {
      decodeJwtPayload(pat);
      accessToken = pat;
      console.log("   Using bootstrap PAT as access token (for shape verification only).");
    } catch {
      throw new Error(
        "Cannot mint a test token. Enable password grant in Zitadel dev settings or " +
        "configure a service-user PAT that is a JWT."
      );
    }
  }

  // 3. Introspect to get full claims (more reliable than decoding the JWT for claim names)
  console.log("3. Introspecting token for claim shape…");
  const introspectRes = await fetch(`${issuer}/oauth/v2/introspect`, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ token: accessToken }),
    ...(() => {
      const creds = Buffer.from(`${gatewayClientId}:${gatewaySecret}`).toString("base64");
      return { headers: { Authorization: `Basic ${creds}`, "Content-Type": "application/x-www-form-urlencoded" } };
    })(),
  });
  if (!introspectRes.ok) {
    const txt = await introspectRes.text();
    throw new Error(`Introspection failed: HTTP ${introspectRes.status} — ${txt}`);
  }
  const introspectData = await introspectRes.json();

  // 4. Decode the JWT payload as well
  let jwtPayload = {};
  try {
    jwtPayload = decodeJwtPayload(accessToken);
  } catch {}

  const claims = { ...introspectData, ...jwtPayload };

  // 5. Assert required claims
  const errors = [];
  const required = { iss: issuer, sub: null /* any non-empty */ };
  for (const [k, expected] of Object.entries(required)) {
    if (!claims[k]) {
      errors.push(`Missing claim: ${k}`);
    } else if (expected && claims[k] !== expected) {
      errors.push(`Claim ${k}: expected "${expected}", got "${claims[k]}"`);
    }
  }

  if (!claims.aud && !claims.aud?.length) {
    errors.push("Missing claim: aud");
  }
  if (!claims[orgClaim]) {
    errors.push(`Missing org claim: ${orgClaim}`);
  }
  // roles claim may be absent if the test user has no project roles; warn instead of error
  if (!claims[rolesClaim]) {
    console.warn(`   ⚠ Roles claim absent: ${rolesClaim} (user may have no project roles assigned)`);
  }

  if (errors.length > 0) {
    console.error("\nToken shape assertion FAILED:");
    for (const e of errors) console.error(` - ${e}`);
    // Cleanup
    try { await mgmtApi("DELETE", `/users/${userId}`); } catch {}
    process.exit(1);
  }

  // 6. Write fixture
  const fixture = {
    assertedAt: new Date().toISOString(),
    issuer,
    claimKeys: {
      iss: "iss",
      sub: "sub",
      aud: "aud",
      orgId: orgClaim,
      projectRoles: rolesClaim,
    },
    sampleClaims: {
      iss: claims.iss,
      sub: claims.sub,
      aud: claims.aud,
      [orgClaim]: claims[orgClaim],
    },
  };

  mkdirSync("tests/fixtures", { recursive: true });
  writeFileSync("tests/fixtures/zitadel-token-shape.json", JSON.stringify(fixture, null, 2) + "\n");
  console.log("   Written: tests/fixtures/zitadel-token-shape.json");

  // 7. Cleanup throwaway user
  try {
    await mgmtApi("DELETE", `/users/${userId}`);
    console.log("   Throwaway user deleted.");
  } catch (e) {
    console.warn(`   Could not delete user ${userId}: ${e.message}`);
  }

  console.log("\n✅ Token shape assertion passed.\n");
  console.log("   Verified claims:");
  console.log(`     iss = ${claims.iss}`);
  console.log(`     sub = ${claims.sub}`);
  console.log(`     aud = ${JSON.stringify(claims.aud)}`);
  console.log(`     ${orgClaim} = ${claims[orgClaim]}`);
}

main().catch((e) => {
  console.error("Token-shape assertion failed:", e.message);
  process.exit(1);
});
