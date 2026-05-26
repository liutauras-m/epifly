/**
 * dokploy/lib/dotenv.mjs
 * Pure .env file parsing and rendering helpers.
 * Zero dependencies.
 */

/**
 * Parse a dotenv-format string into a plain object.
 * Handles: bare values, double/single-quoted values, blank lines, # comments.
 *
 * @param {string} text
 * @returns {Record<string, string>}
 */
export function parseDotenv(text) {
  /** @type {Record<string, string>} */
  const out = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq < 0) continue;
    const k = line.slice(0, eq).trim();
    let v = line.slice(eq + 1).trim();
    if (
      (v.startsWith('"') && v.endsWith('"')) ||
      (v.startsWith("'") && v.endsWith("'"))
    ) {
      v = v.slice(1, -1);
    }
    out[k] = v;
  }
  return out;
}

/**
 * Render a merged env object back to dotenv text, preserving comments and key
 * order from the prior text where possible. New keys are appended.
 *
 * @param {Record<string, string>} merged     Final key→value map to write.
 * @param {string} priorText                  Existing dotenv text (for ordering/comments).
 * @returns {string}
 */
export function renderDotenv(merged, priorText) {
  const seen = new Set();
  const out = [];
  for (const raw of priorText.split(/\r?\n/)) {
    const m = raw.match(/^\s*([A-Z][A-Z0-9_]*)\s*=/);
    if (m) {
      const k = m[1];
      if (k in merged) {
        out.push(`${k}=${merged[k]}`);
        seen.add(k);
      } else {
        out.push(raw); // key not in merged — preserve as-is
      }
    } else {
      out.push(raw); // comment or blank line
    }
  }
  for (const k of Object.keys(merged)) {
    if (!seen.has(k)) out.push(`${k}=${merged[k]}`);
  }
  return out.join("\n");
}

/**
 * Returns true if the key name looks like a secret that should be masked
 * in log output.
 *
 * @param {string} k
 * @returns {boolean}
 */
export function isSecret(k) {
  return /PASSWORD|SECRET|TOKEN/.test(k) || /_KEY(_|$)/.test(k);
}
