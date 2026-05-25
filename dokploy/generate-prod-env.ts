#!/usr/bin/env -S node --import=tsx
/**
 * generate-prod-env.ts
 * ─────────────────────────────────────────────────────────────────────────────
 * Generates a production-ready Dokploy `.env` file by taking `.env.example`
 * as the template and replacing every `changeme_*` placeholder with a fresh,
 * cryptographically-strong secret of the correct shape/length.
 *
 * Usage:
 *   pnpm tsx dokploy/generate-prod-env.ts                  # → dokploy/.env.production
 *   pnpm tsx dokploy/generate-prod-env.ts --out path.env   # custom output
 *   pnpm tsx dokploy/generate-prod-env.ts --force          # overwrite existing
 *   APP_DOMAIN=prod.example.com pnpm tsx … --force         # override any var
 *
 * Pass-through overrides: any environment variable already set in the calling
 * shell wins over both the template default AND the generated secret. Use this
 * to inject real values (OPENAI_API_KEY, STRIPE_SECRET_KEY, LAGO_API_KEY, …)
 * without committing them.
 *
 * Output is restricted to mode 0600 (owner-only read/write).
 */

import { generateKeyPairSync, randomBytes } from "node:crypto";
import { existsSync, readFileSync, writeFileSync, chmodSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TEMPLATE_PATH = resolve(HERE, ".env.example");

// ── Argv parsing ────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const force = argv.includes("--force") || argv.includes("-f");
const outIdx = argv.findIndex((a) => a === "--out" || a === "-o");
const outArg = outIdx >= 0 ? argv[outIdx + 1] : undefined;
const OUT_PATH = resolve(HERE, outArg ?? ".env.production");

// ── Secret generators ───────────────────────────────────────────────────────

/** Hex string of `bytes` bytes (length = bytes * 2). */
const hex = (bytes: number): string => randomBytes(bytes).toString("hex");

/** URL-safe base64 of `bytes` bytes, trimmed to `length` chars when given. */
const b64url = (bytes: number, length?: number): string => {
	const s = randomBytes(bytes).toString("base64url");
	return length ? s.slice(0, length) : s;
};

/** Exact-length alphanumeric+symbol password (avoids shell-unsafe chars). */
const password = (length: number): string => {
	const alphabet =
		"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789-_";
	const out: string[] = [];
	const buf = randomBytes(length * 2);
	for (let i = 0; out.length < length && i < buf.length; i++) {
		const idx = buf[i] % alphabet.length;
		out.push(alphabet[idx]);
	}
	return out.join("");
};

/** Generate a 2048-bit RSA private key, base64-encoded for Lago (`LAGO_RSA_PRIVATE_KEY` canonical format — single line, safe in env files). */
const rsaPrivateKeyBase64 = (): string => {
	const { privateKey } = generateKeyPairSync("rsa", {
		modulusLength: 2048,
		privateKeyEncoding: { type: "pkcs8", format: "pem" },
		publicKeyEncoding: { type: "spki", format: "pem" },
	});
	return Buffer.from(privateKey, "utf8").toString("base64");
};

// ── Per-variable generation rules ───────────────────────────────────────────
// Keys NOT listed here are left at their template value (unless overridden
// by process.env). Values listed here are regenerated whenever the template
// value starts with `changeme_` OR when the template value is empty.
const generators: Record<string, () => string> = {
	POSTGRES_PASSWORD: () => password(40),
	ZITADEL_MASTERKEY: () => password(32), // Zitadel requires exactly 32 chars
	LAGO_SECRET_KEY_BASE: () => hex(64), // 128-char hex (≥ Rails default of 64)
	LAGO_ENCRYPTION_DET_KEY: () => password(32),
	LAGO_ENCRYPTION_SALT: () => password(32),
	LAGO_ENCRYPTION_KEY: () => password(32),
	LAGO_RSA_PRIVATE_KEY: () => rsaPrivateKeyBase64(),
	// RustFS S3 root credentials — deliberately NOT using the AWS `AKIA…` prefix
	// to avoid false-positive triggers in secret scanners.
	AWS_ACCESS_KEY_ID: () => `rfs_${b64url(12).replace(/[-_]/g, "").toUpperCase().slice(0, 16)}`,
	AWS_SECRET_ACCESS_KEY: () => b64url(30, 40),
	UI_SESSION_KEY: () => hex(32), // 64-char hex (32 bytes of entropy)
	PLATFORM_ADMIN_TOKEN: () => `pat_${b64url(32)}`,
	RUSTFS_IAM_ENC_KEY: () => password(32),
	RUSTFS_WEBHOOK_SECRET: () => b64url(32),
};

// Optional pass-through vars: never auto-generated, only filled from
// process.env if the user supplied them.
const passthrough = [
	"ZITADEL_CLIENT_ID",
	"LAGO_API_KEY",
	"STRIPE_SECRET_KEY",
	"OPENAI_API_KEY",
	"ANTHROPIC_API_KEY",
];

// ── Main ────────────────────────────────────────────────────────────────────
if (!existsSync(TEMPLATE_PATH)) {
	console.error(`✘ Template not found: ${TEMPLATE_PATH}`);
	process.exit(1);
}
if (existsSync(OUT_PATH) && !force) {
	console.error(
		`✘ ${OUT_PATH} already exists. Refusing to overwrite. Pass --force to replace.`,
	);
	process.exit(1);
}

const template = readFileSync(TEMPLATE_PATH, "utf8");
const lines = template.split("\n");
const generated: string[] = [];
const filled: string[] = [];

const rewritten = lines.map((line) => {
	const match = /^([A-Z][A-Z0-9_]*)=(.*)$/.exec(line);
	if (!match) return line; // comments, blank lines, etc.
	const [, key, currentValue] = match;

	// 1. process.env override always wins.
	const envOverride = process.env[key];
	if (envOverride !== undefined && envOverride !== "") {
		filled.push(key);
		return `${key}=${envOverride}`;
	}

	// 2. Generated secret for known keys with placeholder/empty values.
	const needsGen =
		currentValue.startsWith("changeme_") ||
		(currentValue === "" && key in generators);
	if (needsGen && key in generators) {
		generated.push(key);
		return `${key}=${generators[key]()}`;
	}

	// 3. Empty passthrough — leave blank but record so user knows.
	if (currentValue === "" && passthrough.includes(key)) {
		return line; // unchanged; user must fill in via Dokploy UI
	}

	// 4. Everything else (APP_DOMAIN, TRAEFIK_*, POSTGRES_USER, etc.) kept.
	return line;
});

// Banner replacement — make the file self-explanatory.
const banner =
	[
		"# ─────────────────────────────────────────────────────────────────────────────",
		`# Conusai — Generated production environment`,
		`# Generated: ${new Date().toISOString()}`,
		`# Source:    dokploy/.env.example`,
		`#`,
		`# Paste into Dokploy → Project → Settings → Shared Environment.`,
		`# DO NOT COMMIT this file. It is git-ignored via dokploy/.gitignore.`,
		`# ─────────────────────────────────────────────────────────────────────────────`,
		"",
	].join("\n");

const output =
	banner +
	rewritten
		.filter((_, i, arr) => {
			// Strip the original template banner (first contiguous block of comments).
			if (i > 8) return true;
			return !/^# /.test(arr[i]) && arr[i].trim() !== "";
		})
		.join("\n") +
	(rewritten.at(-1) === "" ? "" : "\n");

writeFileSync(OUT_PATH, output, { encoding: "utf8" });
chmodSync(OUT_PATH, 0o600);

console.log(`✔ Wrote ${OUT_PATH} (mode 0600)`);
console.log(`  generated:    ${generated.length} secret(s) — ${generated.join(", ")}`);
if (filled.length) {
	console.log(`  from env:     ${filled.length} value(s) — ${filled.join(", ")}`);
}
const blankPassthrough = passthrough.filter((k) => !filled.includes(k));
if (blankPassthrough.length) {
	console.log(
		`  still blank:  ${blankPassthrough.join(", ")}\n` +
			`               (set via process.env or fill in Dokploy after first deploy)`,
	);
}
