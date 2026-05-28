#!/usr/bin/env node
/**
 * scripts/zitadel-bootstrap.mjs
 *
 * Idempotent Zitadel bootstrap for Epifly (Plan v5.1, steps Z.1–Z.9).
 * Run once per environment (dev / staging / prod) with a service-user PAT that
 * has IAM_OWNER permissions.
 *
 * Required env vars (set in .env.local — never commit):
 *   ZITADEL_ISSUER          https://auth.yourdomain.com  (no trailing slash)
 *   ZITADEL_BOOTSTRAP_PAT   service-account PAT with IAM_OWNER
 *   WEB_HOST                e.g. app.epifly.app (no protocol)
 *   AUTH_REDIRECT_BASE      https://auth.epifly.app (universal-link host)
 *
 * Outputs (printed + written to .env.zitadel for sourcing):
 *   ZITADEL_WEB_CLIENT_ID
 *   ZITADEL_NATIVE_CLIENT_ID
 *   ZITADEL_GATEWAY_CLIENT_ID
 *   ZITADEL_GATEWAY_INTROSPECT_SECRET
 */

import { writeFileSync, existsSync, readFileSync } from "node:fs";
import { createInterface } from "node:readline";

const issuer = requireEnv("ZITADEL_ISSUER");
const pat = requireEnv("ZITADEL_BOOTSTRAP_PAT");
const webHost = requireEnv("WEB_HOST");
const authRedirectBase = process.env.AUTH_REDIRECT_BASE ?? `https://auth.${webHost}`;

const mgmt = `${issuer}/management/v1`;
const admin = `${issuer}/admin/v1`;
const headers = {
  "Authorization": `Bearer ${pat}`,
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

async function api(method, url, body) {
  const res = await fetch(url, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  if (!res.ok) {
    throw new Error(`${method} ${url} → HTTP ${res.status}: ${text}`);
  }
  return text ? JSON.parse(text) : {};
}

async function getOrCreate(listFn, checkFn, createFn, label) {
  const existing = await listFn();
  if (existing) {
    console.log(`  ✓ ${label}: ${existing}`);
    return existing;
  }
  const created = await createFn();
  console.log(`  + ${label}: ${created}`);
  return created;
}

// ── Z.3 Project ───────────────────────────────────────────────────────────────

async function ensureProject() {
  const projectName = "epifly";
  try {
    const res = await api("POST", `${mgmt}/projects/_search`, { query: { offset: "0", limit: 100 } });
    const found = (res.result ?? []).find((p) => p.name === projectName);
    if (found) return found.id;
  } catch {}

  const res = await api("POST", `${mgmt}/projects`, {
    name: projectName,
    projectRoleAssertion: true,
    projectRoleCheck: true,
  });
  return res.id;
}

// ── Z.4 Web app (PKCE, no secret) ─────────────────────────────────────────────

async function ensureWebApp(projectId) {
  const appName = "epifly-web";
  const res = await api("POST", `${mgmt}/projects/${projectId}/apps/_search`, {
    query: { offset: "0", limit: 100 },
  });
  const existing = (res.result ?? []).find((a) => a.name === appName);
  if (existing) return existing.clientId;

  const app = await api("POST", `${mgmt}/projects/${projectId}/apps/oidc`, {
    name: appName,
    redirectUris: [`https://${webHost}/auth/callback`],
    postLogoutRedirectUris: [`https://${webHost}/`],
    responseTypes: ["OIDC_RESPONSE_TYPE_CODE"],
    grantTypes: ["OIDC_GRANT_TYPE_AUTHORIZATION_CODE", "OIDC_GRANT_TYPE_REFRESH_TOKEN"],
    appType: "OIDC_APP_TYPE_WEB",
    authMethodType: "OIDC_AUTH_METHOD_TYPE_NONE", // PKCE, no secret
    accessTokenType: "OIDC_TOKEN_TYPE_JWT",
    accessTokenRoleAssertion: true,
    idTokenRoleAssertion: true,
    idTokenUserinfoAssertion: true,
  });
  return app.clientId;
}

// ── Z.5 Native app (PKCE, public) ─────────────────────────────────────────────

async function ensureNativeApp(projectId) {
  const appName = "epifly-native";
  const res = await api("POST", `${mgmt}/projects/${projectId}/apps/_search`, {
    query: { offset: "0", limit: 100 },
  });
  const existing = (res.result ?? []).find((a) => a.name === appName);
  if (existing) return existing.clientId;

  const app = await api("POST", `${mgmt}/projects/${projectId}/apps/oidc`, {
    name: appName,
    redirectUris: [
      `${authRedirectBase}/native/callback`,     // Universal Link (prod)
      "epifly://auth/callback",                   // custom scheme fallback
      "http://127.0.0.1:53682/callback",          // desktop loopback
    ],
    postLogoutRedirectUris: [`${authRedirectBase}/`],
    responseTypes: ["OIDC_RESPONSE_TYPE_CODE"],
    grantTypes: ["OIDC_GRANT_TYPE_AUTHORIZATION_CODE", "OIDC_GRANT_TYPE_REFRESH_TOKEN"],
    appType: "OIDC_APP_TYPE_NATIVE",
    authMethodType: "OIDC_AUTH_METHOD_TYPE_NONE",
    accessTokenType: "OIDC_TOKEN_TYPE_JWT",
    accessTokenRoleAssertion: true,
    idTokenRoleAssertion: true,
    idTokenUserinfoAssertion: true,
  });
  return app.clientId;
}

// ── Z.6 Gateway API app ────────────────────────────────────────────────────────

async function ensureGatewayApp(projectId) {
  const appName = "epifly-gateway";
  const res = await api("POST", `${mgmt}/projects/${projectId}/apps/_search`, {
    query: { offset: "0", limit: 100 },
  });
  const existing = (res.result ?? []).find((a) => a.name === appName);
  if (existing) {
    // Re-fetch clientId; secret not re-returned — user must store it on first create
    return { clientId: existing.clientId, clientSecret: null };
  }

  const app = await api("POST", `${mgmt}/projects/${projectId}/apps/api`, {
    name: appName,
    authMethodType: "API_AUTH_METHOD_TYPE_BASIC",
  });
  return { clientId: app.clientId, clientSecret: app.clientSecret };
}

// ── Z.7 Claim mappings ─────────────────────────────────────────────────────────

async function ensureProjectRoleMapping(projectId) {
  // Role actions via the Zitadel action system (project-level claim mapping)
  // Zitadel automatically includes urn:zitadel:iam:org:project:roles when
  // projectRoleAssertion=true on the project + apps — no extra action needed.
  // org_id claim (urn:zitadel:iam:user:resourceowner:id) is standard in Zitadel access tokens.
  console.log("  ✓ Claim mappings: Zitadel built-in (projectRoleAssertion=true on project)");
}

// ── Z.7 Project roles ─────────────────────────────────────────────────────────

async function ensureProjectRoles(projectId) {
  const roles = [
    { roleKey: "tenant.admin", displayName: "Tenant Admin", group: "tenant" },
    { roleKey: "tenant.member", displayName: "Tenant Member", group: "tenant" },
    { roleKey: "platform.admin", displayName: "Platform Admin", group: "platform" },
  ];

  const res = await api("POST", `${mgmt}/projects/${projectId}/roles/_search`, {
    query: { offset: "0", limit: 100 },
  });
  const existing = new Set((res.result ?? []).map((r) => r.roleKey));

  for (const role of roles) {
    if (existing.has(role.roleKey)) {
      console.log(`  ✓ Role: ${role.roleKey}`);
    } else {
      await api("POST", `${mgmt}/projects/${projectId}/roles`, role);
      console.log(`  + Role: ${role.roleKey}`);
    }
  }
}

// ── Z.8 Passwordless / WebAuthn policy ────────────────────────────────────────

async function ensurePasswordlessPolicy() {
  try {
    await api("PUT", `${mgmt}/policies/login`, {
      allowUsernamePassword: true,
      allowRegister: true,
      allowExternalIdp: false,
      forceMfa: false,
      passwordlessType: "PASSWORDLESS_TYPE_ALLOWED",
      hidePasswordReset: false,
    });
    console.log("  ✓ Login policy: passwordless allowed");
  } catch (e) {
    console.warn(`  ⚠ Could not set login policy: ${e.message}`);
  }
}

// ── Main ──────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`\nZitadel bootstrap — issuer: ${issuer}\n`);

  // Z.3
  console.log("Z.3 Project…");
  const projectId = await ensureProject();
  console.log(`  Project ID: ${projectId}`);

  // Z.4
  console.log("Z.4 Web app (epifly-web)…");
  const webClientId = await ensureWebApp(projectId);

  // Z.5
  console.log("Z.5 Native app (epifly-native)…");
  const nativeClientId = await ensureNativeApp(projectId);

  // Z.6
  console.log("Z.6 Gateway API app (epifly-gateway)…");
  const { clientId: gatewayClientId, clientSecret: gatewaySecret } =
    await ensureGatewayApp(projectId);
  if (!gatewaySecret) {
    console.log(
      "  ⚠  Gateway app already existed — client secret not re-shown. " +
        "Check .env.zitadel if you stored it, or delete + re-create the app."
    );
  }

  // Z.7
  console.log("Z.7 Project roles + claim mappings…");
  await ensureProjectRoles(projectId);
  await ensureProjectRoleMapping(projectId);

  // Z.8
  console.log("Z.8 Passwordless policy…");
  await ensurePasswordlessPolicy();

  // Z.9 — Write .env.zitadel
  const audience = `${projectId}`;
  const lines = [
    `# Auto-generated by scripts/zitadel-bootstrap.mjs — DO NOT commit`,
    `ZITADEL_ISSUER=${issuer}`,
    `ZITADEL_WEB_CLIENT_ID=${webClientId}`,
    `ZITADEL_NATIVE_CLIENT_ID=${nativeClientId}`,
    `ZITADEL_GATEWAY_CLIENT_ID=${gatewayClientId}`,
    gatewaySecret ? `ZITADEL_GATEWAY_INTROSPECT_SECRET=${gatewaySecret}` : "# ZITADEL_GATEWAY_INTROSPECT_SECRET=<set manually if needed>",
    `ZITADEL_AUDIENCE=${audience}`,
    `ZITADEL_TOKEN_VERIFY_MODE=jwks`,
    `AUTH_REDIRECT_BASE=${authRedirectBase}`,
    `AUTH_AUTO_PROVISION_TENANTS=false`,
    `# AUTH_SESSION_PEPPER=$(openssl rand -base64 48)`,
    `# AUTH_SESSION_PEPPER=<fill in: openssl rand -base64 48>`,
  ];
  const envPath = ".env.zitadel";
  writeFileSync(envPath, lines.join("\n") + "\n");

  console.log(`\n✅ Bootstrap complete. Written to ${envPath}`);
  console.log("   Next: fill in AUTH_SESSION_PEPPER with: openssl rand -base64 48");
  console.log("   Then: run scripts/zitadel-assert-token-shape.mjs to verify Z.10");
}

main().catch((e) => {
  console.error("Bootstrap failed:", e);
  process.exit(1);
});
