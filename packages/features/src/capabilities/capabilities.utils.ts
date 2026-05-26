import type { CapabilityCard } from "@conusai/types";

/** Returns capabilities matching a search string (client-side filter). */
export function filterCapabilities(
  capabilities: CapabilityCard[],
  q: string
): CapabilityCard[] {
  const lower = q.toLowerCase().trim();
  if (!lower) return capabilities;
  return capabilities.filter(c =>
    c.name?.toLowerCase().includes(lower) ||
    c.description?.toLowerCase().includes(lower)
  );
}
