/**
 * Structured auth audit log.
 *
 * All events go to stdout as newline-delimited JSON with `target: "audit"`.
 * This stream is intentionally separate from the application log and must
 * NEVER contain: access_token, refresh_token, id_token, code, email,
 * full IP address, Authorization header, Cookie header, or raw OIDC claims.
 */

function ipPrefix(ip: string | undefined): string {
  if (!ip) return "unknown";
  // IPv4: keep first two octets (x.x.0.0)
  const v4 = ip.match(/^(\d{1,3})\.(\d{1,3})\.\d{1,3}\.\d{1,3}$/);
  if (v4) return `${v4[1]}.${v4[2]}.0.0`;
  // IPv6: keep first two groups
  const v6parts = ip.split(":");
  if (v6parts.length >= 2) return `${v6parts[0]}:${v6parts[1]}::/32`;
  return "unknown";
}

function uaClass(ua: string | undefined): string {
  if (!ua) return "unknown";
  if (/bot|crawl|spider|slurp|mediapartners/i.test(ua)) return "bot";
  if (/mobile|android|iphone|ipad/i.test(ua)) return "mobile";
  if (/tauri|electron/i.test(ua)) return "native";
  return "browser";
}

interface BaseEvent {
  event: string;
  ts: string;
}

function emit(fields: BaseEvent & Record<string, string | boolean | undefined>): void {
  // Ensure nothing leaks — redact any field whose key suggests a token/secret
  const safe: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(fields)) {
    if (/token|secret|code|password|credential|authorization|cookie/i.test(k)) {
      safe[k] = "[REDACTED]";
    } else {
      safe[k] = v;
    }
  }
  // Write to stdout so log aggregators can route by target field
  process.stdout.write(JSON.stringify({ target: "audit", ...safe }) + "\n");
}

export function auditLoginSuccess(params: {
  iss: string;
  sub: string;
  orgId: string;
  idp?: string;
  rawIp?: string;
  rawUa?: string;
}): void {
  emit({
    event: "auth.login.success",
    ts: new Date().toISOString(),
    iss: params.iss,
    sub: params.sub,
    org_id: params.orgId,
    idp: params.idp ?? "zitadel",
    ip_prefix: ipPrefix(params.rawIp),
    ua_class: uaClass(params.rawUa),
  });
}

export function auditLoginFailure(params: {
  reason: string;
  rawIp?: string;
  rawUa?: string;
}): void {
  emit({
    event: "auth.login.failure",
    ts: new Date().toISOString(),
    reason: params.reason,
    ip_prefix: ipPrefix(params.rawIp),
    ua_class: uaClass(params.rawUa),
  });
}

export function auditLogout(params: { iss: string; sub: string }): void {
  emit({
    event: "auth.logout",
    ts: new Date().toISOString(),
    iss: params.iss,
    sub: params.sub,
  });
}

export function auditRefreshFailure(params: { reason: string }): void {
  emit({
    event: "auth.refresh.failure",
    ts: new Date().toISOString(),
    reason: params.reason,
  });
}

export function auditTenantBindingFailure(params: {
  iss: string;
  orgId: string;
  reason: string;
}): void {
  emit({
    event: "auth.tenant_binding.failure",
    ts: new Date().toISOString(),
    iss: params.iss,
    org_id: params.orgId,
    reason: params.reason,
  });
}
