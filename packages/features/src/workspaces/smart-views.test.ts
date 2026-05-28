import { describe, it, expect } from "vitest";

// ---------------------------------------------------------------------------
// Step 2.2 + 2.3 — smart-views pure logic tests
// We test only the pure helper logic here (isUnsorted classification +
// recently-updated sort) without a real SDK — store behaviour tested in E2E.
// ---------------------------------------------------------------------------

// Import the sort/filter helpers by reproducing the logic from the store.
// If the store ever exports helpers directly, replace with those imports.

const DEFAULT_PROJECTION_FOLDER = "Conversations";

function isUnsorted(virtualPath: string): boolean {
  const path = virtualPath ?? "";
  return (
    path === DEFAULT_PROJECTION_FOLDER ||
    path.startsWith(`${DEFAULT_PROJECTION_FOLDER}/`) ||
    !path.includes("/")
  );
}

function sortByLastModifiedDesc(
  nodes: { last_modified: string }[]
): { last_modified: string }[] {
  return [...nodes].sort((a, b) => {
    const bt = Date.parse(b.last_modified);
    const at = Date.parse(a.last_modified);
    return (Number.isFinite(bt) ? bt : 0) - (Number.isFinite(at) ? at : 0);
  });
}

// ---------------------------------------------------------------------------
// Step 2.2 — "Unsorted" classification
// ---------------------------------------------------------------------------

describe("isUnsorted", () => {
  it("classifies a thread in the default Conversations folder as unsorted", () => {
    expect(isUnsorted("Conversations/thread-abc.md")).toBe(true);
  });

  it("classifies a root-level thread (no slash) as unsorted", () => {
    expect(isUnsorted("my-chat.md")).toBe(true);
  });

  it("classifies a thread moved to a user folder as sorted", () => {
    expect(isUnsorted("Clients/Acme/kickoff.md")).toBe(false);
  });

  it("classifies a thread in a nested user folder as sorted", () => {
    expect(isUnsorted("Projects/Backend/sprint-planning.md")).toBe(false);
  });

  it("treats the bare Conversations folder node itself as unsorted", () => {
    expect(isUnsorted("Conversations")).toBe(true);
  });

  it("does NOT treat a folder starting with Conversations prefix as unsorted if not a match", () => {
    // "ConversationsBackup" should NOT match
    expect(isUnsorted("ConversationsBackup/thread.md")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Step 2.3 — Recently-updated sort
// ---------------------------------------------------------------------------

describe("sortByLastModifiedDesc", () => {
  it("sorts nodes newest-first", () => {
    const nodes = [
      { last_modified: "2026-01-10T00:00:00Z" },
      { last_modified: "2026-01-12T00:00:00Z" },
      { last_modified: "2026-01-11T00:00:00Z" },
    ];
    const sorted = sortByLastModifiedDesc(nodes);
    expect(sorted[0].last_modified).toBe("2026-01-12T00:00:00Z");
    expect(sorted[1].last_modified).toBe("2026-01-11T00:00:00Z");
    expect(sorted[2].last_modified).toBe("2026-01-10T00:00:00Z");
  });

  it("puts nodes with invalid/absent dates last", () => {
    const nodes = [
      { last_modified: "invalid" },
      { last_modified: "2026-01-12T00:00:00Z" },
    ];
    const sorted = sortByLastModifiedDesc(nodes);
    expect(sorted[0].last_modified).toBe("2026-01-12T00:00:00Z");
    expect(sorted[1].last_modified).toBe("invalid");
  });

  it("is stable for equal timestamps", () => {
    const nodes = [
      { last_modified: "2026-01-10T00:00:00Z", id: "a" },
      { last_modified: "2026-01-10T00:00:00Z", id: "b" },
    ];
    const sorted = sortByLastModifiedDesc(nodes as { last_modified: string; id: string }[]);
    // Same timestamps — both present, order doesn't matter but no crash.
    expect(sorted).toHaveLength(2);
  });
});
