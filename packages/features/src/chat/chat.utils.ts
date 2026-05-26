/**
 * Pure, side-effect-free chat utilities.
 */

/** Returns a truncated preview string suitable for thread list titles. */
export function previewContent(content: string, maxLength = 80): string {
  const trimmed = content.trim();
  if (trimmed.length <= maxLength) return trimmed;
  return trimmed.slice(0, maxLength).trimEnd() + "…";
}

/** Returns true if a message role is assistant. */
export function isAssistant(role: "user" | "assistant"): boolean {
  return role === "assistant";
}
