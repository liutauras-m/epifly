/**
 * Pinned capability IDs, persisted to localStorage (Phase 4.7).
 *
 * Platform-agnostic: used by CapabilityBrowser to persist the user's pinned
 * capabilities across sessions. The chip rail reads `pins.ids`, the browser
 * calls `pins.toggle(id)` on chip click.
 *
 * Storage key: "conusai_pins"
 * Cap: 50 (capabilities are user-curated; an unlimited set is never practical)
 */
const STORAGE_KEY = 'conusai_pins';
const MAX = 50;

function loadFromStorage(): string[] {
  if (typeof window === 'undefined') return [];
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
  } catch {
    return [];
  }
}

let ids = $state<string[]>(loadFromStorage());

function persist(next: string[]) {
  if (typeof window !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  }
}

export const pinsStore = {
  /** Reactive list of pinned capability IDs (stable order, most-recently pinned first). */
  get ids(): string[] {
    return ids;
  },

  /** Returns true if the given capability ID is currently pinned. */
  has(id: string): boolean {
    return ids.includes(id);
  },

  /** Pin a capability. No-op if already pinned. Caps at MAX. */
  pin(id: string) {
    if (ids.includes(id)) return;
    ids = [id, ...ids].slice(0, MAX);
    persist(ids);
  },

  /** Unpin a capability. No-op if not pinned. */
  unpin(id: string) {
    ids = ids.filter((i) => i !== id);
    persist(ids);
  },

  /** Toggle: pins if not pinned, unpins if pinned. */
  toggle(id: string) {
    if (ids.includes(id)) {
      this.unpin(id);
    } else {
      this.pin(id);
    }
  },

  /** Clear all pins. */
  clear() {
    ids = [];
    if (typeof window !== 'undefined') localStorage.removeItem(STORAGE_KEY);
  },
};
