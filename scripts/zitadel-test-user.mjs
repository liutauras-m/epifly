#!/usr/bin/env node
/**
 * scripts/zitadel-test-user.mjs
 *
 * Provisions a deterministic test user via the Zitadel Management API.
 * Used by CI and Phase 8 acceptance tests to create mgmt-API users instead
 * of relying on brittle UI registration.
 *
 * Usage:
 *   node scripts/zitadel-test-user.mjs <email> [--role tenant.member] [--delete]
 *
 * Required env vars:
 *   ZITADEL_ISSUER
 *   ZITADEL_BOOTSTRAP_PAT
 *
 * Output: JSON to stdout with { userId, email, password, orgId }
 */

import { randomUUID } from "node:crypto";

const issuer = requireEnv("ZITADEL_ISSUER");
const pat = requireEnv("ZITADEL_BOOTSTRAP_PAT");
const mgmt = `${issuer}/management/v1`;
const mgmtHeaders = { Authorization: `Bearer ${pat}`, "Content-Type": "application/json" };

function requireEnv(name) {
  const v = process.env[name];
  if (!v) { console.error(`Missing: ${name}`); process.exit(1); }
  return v;
}

async function mgmtApi(method, path, body) {
  const res = await fetch(`${mgmt}${path}`, {
    method,
    headers: mgmtHeaders,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`${method} ${path} → HTTP ${res.status}: ${text}`);
  return text ? JSON.parse(text) : {};
}

async function main() {
  const args = process.argv.slice(2);
  const email = args[0];
  const deleteMode = args.includes("--delete");
  const roleArg = args.indexOf("--role");
  const role = roleArg >= 0 ? args[roleArg + 1] : "tenant.member";

  if (!email) {
    console.error("Usage: zitadel-test-user.mjs <email> [--role tenant.member] [--delete]");
    process.exit(1);
  }

  if (deleteMode) {
    // Find user by username and delete
    const searchRes = await mgmtApi("POST", "/users/_search", {
      query: { offset: "0", limit: 1 },
      queries: [{ userNameQuery: { userName: email, method: "TEXT_QUERY_METHOD_EQUALS" } }],
    });
    const user = (searchRes.result ?? [])[0];
    if (!user) {
      console.error(`User not found: ${email}`);
      process.exit(1);
    }
    await mgmtApi("DELETE", `/users/${user.id}`);
    console.log(JSON.stringify({ deleted: true, userId: user.id, email }));
    return;
  }

  // Check if user exists
  const searchRes = await mgmtApi("POST", "/users/_search", {
    query: { offset: "0", limit: 1 },
    queries: [{ userNameQuery: { userName: email, method: "TEXT_QUERY_METHOD_EQUALS" } }],
  });
  const existing = (searchRes.result ?? [])[0];
  if (existing) {
    // User exists — reset password for deterministic test runs
    const newPw = `Test${randomUUID().replace(/-/g, "").slice(0, 12)}!1Aa`;
    await mgmtApi("POST", `/users/${existing.id}/password`, {
      password: { value: newPw, changeRequired: false },
    });
    console.log(JSON.stringify({
      userId: existing.id,
      email,
      password: newPw,
      orgId: existing.details?.resourceOwner,
      existed: true,
    }));
    return;
  }

  // Create new user
  const password = `Test${randomUUID().replace(/-/g, "").slice(0, 12)}!1Aa`;
  const nameParts = email.split("@")[0].split("-").map((p) => p.charAt(0).toUpperCase() + p.slice(1));
  const res = await mgmtApi("POST", "/users/human/_import", {
    userName: email,
    profile: { firstName: nameParts[0] ?? "Test", lastName: nameParts[1] ?? "User" },
    email: { email, isEmailVerified: true },
    password: { value: password, changeRequired: false },
  });

  console.log(JSON.stringify({
    userId: res.userId,
    email,
    password,
    orgId: res.details?.resourceOwner,
    existed: false,
  }));
}

main().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
