import type { ThreadSummary } from "@conusai/sdk";

/** Returns threads sorted by last_active descending. */
export function sortByRecent(threads: ThreadSummary[]): ThreadSummary[] {
  return [...threads].sort((a, b) => {
    const ta = a.last_active ?? "";
    const tb = b.last_active ?? "";
    return tb.localeCompare(ta);
  });
}

/** Returns a display-safe title for a thread. */
export function threadTitle(thread: ThreadSummary, fallback = "Untitled"): string {
  return thread.title?.trim() || fallback;
}
