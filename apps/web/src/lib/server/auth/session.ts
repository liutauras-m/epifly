/**
 * Postgres-backed session store with AEAD-encrypted token blobs.
 *
 * Tokens are encrypted at rest using AES-256-GCM (Node crypto.subtle).
 * The key is derived from AUTH_SESSION_PEPPER via HKDF-SHA-256.
 * The session cookie carries an opaque random id only — never a token.
 *
 * Refresh is single-flight per session row using SELECT … FOR UPDATE.
 */
import postgres from "postgres";
import { env } from "$env/dynamic/private";
import { randomBytes, createHmac } from "node:crypto";
import { subtle } from "node:crypto";

// ── DB connection ──────────────────────────────────────────────────────────────

let _sql: ReturnType<typeof postgres> | null = null;

export function getDb(): ReturnType<typeof postgres> {
  if (!_sql) {
    const url = env.DATABASE_URL;
    if (!url) throw new Error("Missing required env var: DATABASE_URL");
    _sql = postgres(url, { max: 10, idle_timeout: 30, connect_timeout: 10 });
  }
  return _sql;
}

// ── AEAD key (derived from AUTH_SESSION_PEPPER via HKDF) ──────────────────────

let _aesKey: CryptoKey | null = null;

async function getAesKey(): Promise<CryptoKey> {
  if (_aesKey) return _aesKey;

  const pepper = env.AUTH_SESSION_PEPPER;
  if (!pepper) throw new Error("Missing required env var: AUTH_SESSION_PEPPER");

  const rawPepper = Buffer.from(pepper, "base64");
  const keyMaterial = await subtle.importKey("raw", rawPepper, "HKDF", false, ["deriveKey"]);
  _aesKey = await subtle.deriveKey(
    { name: "HKDF", hash: "SHA-256", salt: Buffer.from("epifly-session-v1"), info: new Uint8Array() },
    keyMaterial,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
  return _aesKey;
}

async function encrypt(plaintext: string): Promise<Buffer> {
  const key = await getAesKey();
  const iv = randomBytes(12);
  const ct = await subtle.encrypt({ name: "AES-GCM", iv }, key, Buffer.from(plaintext, "utf8"));
  // Prepend IV (12 bytes) to ciphertext
  return Buffer.concat([iv, Buffer.from(ct)]);
}

async function decrypt(blob: Buffer): Promise<string> {
  const key = await getAesKey();
  const iv = blob.subarray(0, 12);
  const ct = blob.subarray(12);
  const plain = await subtle.decrypt({ name: "AES-GCM", iv }, key, ct);
  return Buffer.from(plain).toString("utf8");
}

// ── Session ID generation ──────────────────────────────────────────────────────

export function generateSessionId(): string {
  return randomBytes(32).toString("base64url");
}

// ── Session types ──────────────────────────────────────────────────────────────

export interface SessionData {
  userIss: string;
  userSub: string;
  tenantOrgId: string;
  displayName: string;
  emailVerified: boolean;
}

export interface SessionRow {
  id: string;
  user_iss: string;
  user_sub: string;
  tenant_org_id: string;
  access_ct: Buffer;
  refresh_ct: Buffer;
  id_token_ct: Buffer | null;
  access_expires_at: Date;
  created_at: Date;
  last_seen_at: Date;
  revoked_at: Date | null;
}

// ── Session creation ───────────────────────────────────────────────────────────

export async function createSession(params: {
  userIss: string;
  userSub: string;
  tenantOrgId: string;
  accessToken: string;
  refreshToken: string;
  idToken?: string;
  accessExpiresAt: Date;
}): Promise<string> {
  const sql = getDb();
  const id = generateSessionId();

  const accessCt = await encrypt(params.accessToken);
  const refreshCt = await encrypt(params.refreshToken);
  const idTokenCt = params.idToken ? await encrypt(params.idToken) : null;

  await sql`
    INSERT INTO auth_sessions
      (id, user_iss, user_sub, tenant_org_id, access_ct, refresh_ct, id_token_ct, access_expires_at)
    VALUES
      (${id}, ${params.userIss}, ${params.userSub}, ${params.tenantOrgId},
       ${accessCt}, ${refreshCt}, ${idTokenCt ?? null}, ${params.accessExpiresAt})
  `;

  return id;
}

// ── Session load ───────────────────────────────────────────────────────────────

export async function loadSession(
  id: string
): Promise<(SessionData & { accessToken: string }) | null> {
  const sql = getDb();
  const rows = await sql<SessionRow[]>`
    SELECT * FROM auth_sessions WHERE id = ${id} AND revoked_at IS NULL LIMIT 1
  `;
  if (!rows.length) return null;

  const row = rows[0];

  // Update last_seen without blocking
  sql`UPDATE auth_sessions SET last_seen_at = now() WHERE id = ${id}`.catch(() => {});

  const accessToken = await decrypt(row.access_ct);

  return {
    userIss: row.user_iss,
    userSub: row.user_sub,
    tenantOrgId: row.tenant_org_id,
    displayName: row.user_sub,
    emailVerified: true,
    accessToken,
  };
}

// ── Token refresh (single-flight via FOR UPDATE) ────────────────────────────────

export async function refreshSession(params: {
  sessionId: string;
  refreshFn: (oldRefreshToken: string) => Promise<{
    accessToken: string;
    refreshToken: string;
    accessExpiresAt: Date;
  }>;
}): Promise<string | null> {
  const sql = getDb();

  return sql.begin(async (tx) => {
    // Acquire a row-level lock — only one concurrent refresh per session id
    const rows = await tx<SessionRow[]>`
      SELECT access_ct, refresh_ct, access_expires_at
        FROM auth_sessions WHERE id = ${params.sessionId} AND revoked_at IS NULL
        FOR UPDATE
    `;
    if (!rows.length) return null;

    const row = rows[0];
    const now = Date.now();
    const expiresAt = row.access_expires_at.getTime();

    // Another concurrent request may have already rotated — reuse its token
    if (expiresAt >= now + 60_000) {
      return decrypt(row.access_ct);
    }

    // Decrypt the refresh token and call the IdP
    const oldRefreshToken = await decrypt(row.refresh_ct);
    let result: { accessToken: string; refreshToken: string; accessExpiresAt: Date };
    try {
      result = await params.refreshFn(oldRefreshToken);
    } catch {
      // invalid_grant or network error → revoke the session
      await tx`UPDATE auth_sessions SET revoked_at = now() WHERE id = ${params.sessionId}`;
      return null;
    }

    const newAccessCt = await encrypt(result.accessToken);
    const newRefreshCt = await encrypt(result.refreshToken);

    await tx`
      UPDATE auth_sessions
         SET access_ct = ${newAccessCt},
             refresh_ct = ${newRefreshCt},
             access_expires_at = ${result.accessExpiresAt},
             last_seen_at = now()
       WHERE id = ${params.sessionId}
    `;

    return result.accessToken;
  });
}

// ── Session revocation ─────────────────────────────────────────────────────────

export async function revokeSession(
  id: string
): Promise<{ idToken: string | null; refreshToken: string | null } | null> {
  const sql = getDb();
  const rows = await sql<{ id_token_ct: Buffer | null; refresh_ct: Buffer }[]>`
    UPDATE auth_sessions
       SET revoked_at = now(), id_token_ct = NULL
     WHERE id = ${id}
    RETURNING id_token_ct, refresh_ct
  `;
  if (!rows.length) return null;

  const row = rows[0];

  let idToken: string | null = null;
  let refreshToken: string | null = null;

  try {
    if (row.id_token_ct) idToken = await decrypt(row.id_token_ct);
  } catch {}

  try {
    refreshToken = await decrypt(row.refresh_ct);
  } catch {}

  return { idToken, refreshToken };
}

// ── OIDC transaction store ─────────────────────────────────────────────────────

export async function createOidcTransaction(params: {
  state: string;
  codeVerifier: string;
  nonce: string;
  returnTo: string;
}): Promise<void> {
  const sql = getDb();
  await sql`
    INSERT INTO auth_oidc_transactions (state, code_verifier, nonce, return_to)
    VALUES (${params.state}, ${params.codeVerifier}, ${params.nonce}, ${params.returnTo})
    ON CONFLICT (state) DO NOTHING
  `;
}

export interface OidcTransaction {
  state: string;
  codeVerifier: string;
  nonce: string;
  returnTo: string;
  consumedAt: Date | null;
}

export async function consumeOidcTransaction(state: string): Promise<OidcTransaction | null> {
  const sql = getDb();
  const rows = await sql<
    { state: string; code_verifier: string; nonce: string; return_to: string; consumed_at: Date | null }[]
  >`
    UPDATE auth_oidc_transactions
       SET consumed_at = now()
     WHERE state = ${state} AND consumed_at IS NULL
    RETURNING state, code_verifier, nonce, return_to, consumed_at
  `;
  if (!rows.length) return null;

  const r = rows[0];
  return {
    state: r.state,
    codeVerifier: r.code_verifier,
    nonce: r.nonce,
    returnTo: r.return_to,
    consumedAt: r.consumed_at,
  };
}

// ── Cleanup (called from cron handler) ────────────────────────────────────────

export async function runSessionCleanup(): Promise<void> {
  const sql = getDb();

  const [txRes, revokedRes, idleRes, hardRes] = await Promise.all([
    sql`DELETE FROM auth_oidc_transactions WHERE created_at < now() - interval '1 day' LIMIT 1000`,
    sql`DELETE FROM auth_sessions WHERE revoked_at IS NOT NULL AND revoked_at < now() - interval '7 days' LIMIT 1000`,
    sql`DELETE FROM auth_sessions WHERE last_seen_at < now() - interval '30 days' LIMIT 1000`,
    sql`DELETE FROM auth_sessions WHERE created_at < now() - interval '90 days' LIMIT 1000`,
  ]);

  console.log(
    `[auth-cleanup] transactions=${txRes.count} revoked=${revokedRes.count} idle=${idleRes.count} hard=${hardRes.count}`
  );
}
