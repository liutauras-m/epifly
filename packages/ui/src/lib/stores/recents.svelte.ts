/**
 * Recently-opened workspace node IDs, persisted to localStorage.
 *
 * Platform-agnostic: used by web sidebar, mobile drawer, and desktop tab strip.
 * Storage key, cap, and read/write are all internal — consumers see a small
 * interface: `ids`, `add(id)`, `list()`, `clear()`.
 */
const STORAGE_KEY = "conusai_recents";
const MAX = 20;

function loadFromStorage(): string[] {
  if (typeof window === "undefined") return [];
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
  } catch {
    return [];
  }
}

let ids = $state<string[]>(loadFromStorage());

export const recentsStore = {
  get ids() {
    return ids;
  },
  add(id: string) {
    ids = [id, ...ids.filter((i) => i !== id)].slice(0, MAX);
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
    }
  },
  list() {
    return ids;
  },
  clear() {
    ids = [];
    if (typeof window !== "undefined") localStorage.removeItem(STORAGE_KEY);
  },
};
