#!/usr/bin/env node
/**
 * CI gate: verify Apple AASA and Android assetlinks.json are served correctly
 * from auth.epifly.app (or AUTH_HOST env override for local testing).
 *
 * Both files must be:
 *   - Served with HTTP 200
 *   - Content-Type: application/json
 *   - No redirect
 *   - Body parseable as JSON with the expected shape
 *
 * Exit 1 if any check fails.
 */

const HOST = process.env.AUTH_HOST ?? "auth.epifly.app";
const APP_ID_PREFIX = process.env.APPLE_TEAM_ID
  ? `${process.env.APPLE_TEAM_ID}.app.epifly.client`
  : null;
const PACKAGE_NAME = process.env.ANDROID_PACKAGE ?? "app.epifly.client";

const checks = [
  {
    name: "Apple AASA",
    url: `https://${HOST}/.well-known/apple-app-site-association`,
    validate(body) {
      const parsed = JSON.parse(body);
      if (!parsed.applinks) throw new Error("missing 'applinks' key");
      const details = parsed.applinks.details ?? [];
      if (!Array.isArray(details) || details.length === 0)
        throw new Error("applinks.details is empty");
      // At least one entry should cover /native/callback
      const hasCb = details.some(
        (d) =>
          Array.isArray(d.paths ?? d.components) &&
          (d.paths ?? []).some((p) => p.includes("/native/callback"))
      );
      if (!hasCb)
        throw new Error(
          "no applinks detail covers /native/callback — Universal Links will not work"
        );
      if (APP_ID_PREFIX) {
        const hasAppId = details.some((d) => d.appID === APP_ID_PREFIX);
        if (!hasAppId)
          throw new Error(`no detail with appID="${APP_ID_PREFIX}" (APPLE_TEAM_ID is set)`);
      }
    },
  },
  {
    name: "Android assetlinks",
    url: `https://${HOST}/.well-known/assetlinks.json`,
    validate(body) {
      const parsed = JSON.parse(body);
      if (!Array.isArray(parsed)) throw new Error("body is not a JSON array");
      const hasPkg = parsed.some((e) => e.target?.package_name === PACKAGE_NAME);
      if (!hasPkg)
        throw new Error(
          `no entry with package_name="${PACKAGE_NAME}" — App Links will not work`
        );
    },
  },
];

let failures = 0;

for (const check of checks) {
  process.stdout.write(`  Checking ${check.name} at ${check.url} … `);
  try {
    const res = await fetch(check.url, { redirect: "error" });
    if (res.status !== 200) {
      throw new Error(`HTTP ${res.status} (expected 200)`);
    }
    const ct = res.headers.get("content-type") ?? "";
    if (!ct.includes("application/json")) {
      throw new Error(`Content-Type "${ct}" is not application/json`);
    }
    const body = await res.text();
    check.validate(body);
    console.log("OK");
  } catch (e) {
    console.log(`FAIL: ${e.message}`);
    failures++;
  }
}

if (failures > 0) {
  console.error(
    `\n[assert-aasa-and-assetlinks] FAIL: ${failures} check(s) failed.\n` +
      `See Phase 4 docs for the exact delivery contract.\n`
  );
  process.exit(1);
} else {
  console.log(`\n[assert-aasa-and-assetlinks] OK — all checks passed.`);
}
