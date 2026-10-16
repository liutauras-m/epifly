/**
 * Recently-opened workspace node IDs, persisted to localStorage in the browser.
 *
 * Platform-agnostic: used by web sidebar, mobile drawer, and desktop tab strip.
 * Consumers see a small interface: `ids`, `add(id)`, `list()`, `clear()`.
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

export function createRecentsStore() {
  let ids = $state<string[]>(loadFromStorage());

  return {
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
}

export type RecentsStore = ReturnType<typeof createRecentsStore>;
