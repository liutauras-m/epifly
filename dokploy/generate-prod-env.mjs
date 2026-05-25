#!/usr/bin/env node
/**
 * generate-prod-env.mjs
 * ─────────────────────────────────────────────────────────────────────────────
 * Generates a production-ready Dokploy `.env` file by taking `.env.example`
 * as the template and replacing every `changeme_*` placeholder with a fresh,
 * cryptographically-strong secret of the correct shape/length.
 *
 * Usage:
 *   node dokploy/generate-prod-env.mjs                  # → dokploy/.env.production
 *   node dokploy/generate-prod-env.mjs --out path.env   # custom output
 *   node dokploy/generate-prod-env.mjs --force          # overwrite existing
 *   APP_DOMAIN=prod.example.com node dokploy/… --force  # override any var
 *
 * Pass-through overrides: any environment variable already set in the calling
 * shell wins over both the template default AND the generated secret. Use this
 * to inject real values (OPENAI_API_KEY, STRIPE_SECRET_KEY, LAGO_API_KEY, …)
 * without committing them.
 *
 * Output is restricted to mode 0600 (owner-only read/write).
 *
 * Zero-dependency: Node 18+ stdlib only. No `tsx`, no `pnpm` required.
 */

import { generateKeyPairSync, randomBytes } from "node:crypto";
import { existsSync, readFileSync, writeFileSync, chmodSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TEMPLATE_PATH = resolve(HERE, ".env.example");
const DOKPLOY_CREDS_PATH = resolve(HERE, ".dokploy");

// ── Load Dokploy operator creds (used by the in-stack `domain-sync` service)
// Format: shell-style `KEY="value"` lines. Gitignored.
// We set them on process.env BEFORE the passthrough loop so they flow into
// the generated `.env.production`. `DOKPLOY_PROJECT_URL` is decomposed into
// `DOKPLOY_ENVIRONMENT_ID` by extracting the path segment after /environment/.
function loadDokployCreds() {
	if (!existsSync(DOKPLOY_CREDS_PATH)) return;
	for (const line of readFileSync(DOKPLOY_CREDS_PATH, "utf8").split("\n")) {
		const m = line.match(/^\s*([A-Z_]+)\s*=\s*"?([^"\n]*?)"?\s*$/);
		if (!m) continue;
		const [, key, value] = m;
		if (key === "DOKPLOY_PROJECT_URL") {
			const envMatch = value.match(/\/environment\/([^/?#]+)/);
			if (envMatch && !process.env.DOKPLOY_ENVIRONMENT_ID) {
				process.env.DOKPLOY_ENVIRONMENT_ID = envMatch[1];
			}
			continue;
		}
		const normalised = key === "DOKPLOY_URL" ? value.replace(/\/+$/, "") : value;
		if (!process.env[key]) process.env[key] = normalised;
	}
}
loadDokployCreds();

// ── Argv parsing ────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const force = argv.includes("--force") || argv.includes("-f");
const outIdx = argv.findIndex((a) => a === "--out" || a === "-o");
const outArg = outIdx >= 0 ? argv[outIdx + 1] : undefined;
const OUT_PATH = resolve(HERE, outArg ?? ".env.production");

// ── Secret generators ───────────────────────────────────────────────────────

/** Hex string of `bytes` bytes (length = bytes * 2). */
const hex = (bytes) => randomBytes(bytes).toString("hex");

/** URL-safe base64 of `bytes` bytes, trimmed to `length` chars when given. */
const b64url = (bytes, length) => {
	const s = randomBytes(bytes).toString("base64url");
	return length ? s.slice(0, length) : s;
};

/** Exact-length alphanumeric+symbol password (avoids shell-unsafe chars). */
const password = (length) => {
	const alphabet =
		"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789-_";
	const out = [];
	const buf = randomBytes(length * 2);
	for (let i = 0; out.length < length && i < buf.length; i++) {
		const idx = buf[i] % alphabet.length;
		out.push(alphabet[idx]);
	}
	return out.join("");
};

/** Generate a 2048-bit RSA private key, base64-encoded for Lago. */
const rsaPrivateKeyBase64 = () => {
	const { privateKey } = generateKeyPairSync("rsa", {
		modulusLength: 2048,
		privateKeyEncoding: { type: "pkcs8", format: "pem" },
		publicKeyEncoding: { type: "spki", format: "pem" },
	});
	return Buffer.from(privateKey, "utf8").toString("base64");
};

// ── Per-variable generation rules ───────────────────────────────────────────
const generators = {
	POSTGRES_PASSWORD: () => password(40),
	ZITADEL_MASTERKEY: () => password(32),
	LAGO_SECRET_KEY_BASE: () => hex(64),
	LAGO_ENCRYPTION_DET_KEY: () => password(32),
	LAGO_ENCRYPTION_SALT: () => password(32),
	LAGO_ENCRYPTION_KEY: () => password(32),
	LAGO_RSA_PRIVATE_KEY: () => rsaPrivateKeyBase64(),
	// b64url(12) yields 16 chars; stripping `-` and `_` can drop us below 16,
	// so generate from a larger pool first and slice afterwards to guarantee 16.
	AWS_ACCESS_KEY_ID: () => `rfs_${b64url(24).replace(/[-_]/g, "").toUpperCase().slice(0, 16)}`,
	AWS_SECRET_ACCESS_KEY: () => b64url(30, 40),
	UI_SESSION_KEY: () => hex(32),
	PLATFORM_ADMIN_TOKEN: () => `pat_${b64url(32)}`,
	RUSTFS_IAM_ENC_KEY: () => password(32),
	RUSTFS_WEBHOOK_SECRET: () => b64url(32),
};

const passthrough = [
	"ZITADEL_CLIENT_ID",
	"LAGO_API_KEY",
	"STRIPE_SECRET_KEY",
	"OPENAI_API_KEY",
	"ANTHROPIC_API_KEY",
	"DOKPLOY_URL",
	"DOKPLOY_ENVIRONMENT_ID",
	"DOKPLOY_API_KEY",
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
const generated = [];
const filled = [];

const rewritten = lines.map((line) => {
	const match = /^([A-Z][A-Z0-9_]*)=(.*)$/.exec(line);
	if (!match) return line;
	const [, key, currentValue] = match;

	const envOverride = process.env[key];
	if (envOverride !== undefined && envOverride !== "") {
		filled.push(key);
		return `${key}=${envOverride}`;
	}

	const needsGen =
		currentValue.startsWith("changeme_") ||
		(currentValue === "" && key in generators);
	if (needsGen && key in generators) {
		generated.push(key);
		return `${key}=${generators[key]()}`;
	}

	if (currentValue === "" && passthrough.includes(key)) {
		return line;
	}

	return line;
});

const banner =
	[
		"# ─────────────────────────────────────────────────────────────────────────────",
		`# Conusai — Generated production environment`,
		`# Generated: ${new Date().toISOString()}`,
		`# Source:    dokploy/.env.example`,
		`#`,
		`# Paste into Dokploy → Project → Settings → Shared Environment.`,
		`# DO NOT COMMIT this file. It is git-ignored via dokploy/.gitignore.`,
		"# ─────────────────────────────────────────────────────────────────────────────",
		"",
	].join("\n");

// Strip only the leading contiguous banner block (comments + blank lines
// before the first real `KEY=value` line) — that's what `banner` replaces.
// The previous heuristic ("drop the first 9 lines if they look like
// comments") silently truncated the inline service-URL comment block when
// the template's banner was shorter than expected.
let firstContentIdx = 0;
for (let i = 0; i < rewritten.length; i++) {
	const t = rewritten[i].trim();
	if (t === "" || t.startsWith("#")) continue;
	firstContentIdx = i;
	break;
}

const output =
	banner +
	rewritten.slice(firstContentIdx).join("\n") +
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
