import { describe, it, expect } from "vitest";
import { toSidebarWorkspaceNode } from "./workspace-adapters.js";
import type { WorkspaceNode } from "@conusai/types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeNode(overrides: Partial<WorkspaceNode> & Pick<WorkspaceNode, "semantic_kind">): WorkspaceNode {
  return {
    id: "node-1",
    parent_id: null,
    kind: "conversation",
    name: "Test node",
    virtual_path: "",
    last_modified: "2026-01-01T00:00:00Z",
    source_type: null,
    source_id: null,
    tags: [],
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Step 0.1 — toSidebarWorkspaceNode branches on semantic_kind
// ---------------------------------------------------------------------------

describe("toSidebarWorkspaceNode — semantic_kind routing", () => {
  it("maps semantic_kind:'thread' to kind:'thread'", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_123",
      virtual_path: "Clients/Acme/Kickoff",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.kind).toBe("thread");
  });

  it("carries threadId from source_id for thread nodes", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_123",
      virtual_path: "Clients/Acme/Kickoff",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.threadId).toBe("t_123");
  });

  it("carries virtualPath for thread nodes", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_123",
      virtual_path: "Clients/Acme/Kickoff",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.virtualPath).toBe("Clients/Acme/Kickoff");
  });

  it("maps semantic_kind:'folder' to kind:'folder'", () => {
    const node = makeNode({ semantic_kind: "folder", kind: "folder" });
    const result = toSidebarWorkspaceNode(node);
    expect(result.kind).toBe("folder");
  });

  it("maps semantic_kind:'file' to kind:'document'", () => {
    const node = makeNode({ semantic_kind: "file", kind: "file" });
    const result = toSidebarWorkspaceNode(node);
    expect(result.kind).toBe("document");
  });

  it("does NOT read storage kind — a 'conversation' storage kind with semantic_kind:'thread' → 'thread'", () => {
    const node = makeNode({
      semantic_kind: "thread",
      kind: "conversation",
      source_id: "t_456",
      virtual_path: "",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.kind).toBe("thread");
  });

  it("threadId is null when source_id is null", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: null,
      virtual_path: "",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.threadId).toBeNull();
  });

  it("virtualPath is empty string when virtual_path is empty", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_789",
      virtual_path: "",
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.virtualPath).toBe("");
  });
});

// ---------------------------------------------------------------------------
// Step 0.4 — work-unit fields pass-through from metadata
// ---------------------------------------------------------------------------

describe("toSidebarWorkspaceNode — work-unit fields", () => {
  it("passes through status from metadata when present", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_1",
      virtual_path: "",
      metadata: { status: "active" },
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.status).toBe("active");
  });

  it("passes through summary from metadata when present", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_1",
      virtual_path: "",
      metadata: { summary: "Discussed Q3 targets." },
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.summary).toBe("Discussed Q3 targets.");
  });

  it("status is undefined when metadata is absent", () => {
    const node = makeNode({ semantic_kind: "file" });
    const result = toSidebarWorkspaceNode(node);
    expect(result.status).toBeUndefined();
  });

  it("passes through tags", () => {
    const node = makeNode({
      semantic_kind: "file",
      tags: ["invoice", "q3"],
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.tags).toEqual(["invoice", "q3"]);
  });

  it("tags default to empty array when absent", () => {
    const node = makeNode({ semantic_kind: "folder", kind: "folder", tags: undefined });
    const result = toSidebarWorkspaceNode(node);
    expect(result.tags).toEqual([]);
  });

  it("carries lastActivityAt from last_modified", () => {
    const ts = "2026-05-28T12:00:00Z";
    const node = makeNode({ semantic_kind: "thread", source_id: "t_1", virtual_path: "", last_modified: ts });
    const result = toSidebarWorkspaceNode(node);
    expect(result.lastActivityAt).toBe(ts);
  });
});

// ---------------------------------------------------------------------------
// Step 6.1 — parentId pass-through (enables folderNodeId derivation in context)
// ---------------------------------------------------------------------------

describe("toSidebarWorkspaceNode — parentId (Step 6.1)", () => {
  it("carries parent_id as parentId for a foldered thread", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_abc",
      virtual_path: "Work/Projects/Chat",
      parent_id: "folder-xyz",
    });
    const result = toSidebarWorkspaceNode(node);
    // The chat page reads result.parentId as the ambient workspaceNodeId.
    expect(result.parentId).toBe("folder-xyz");
  });

  it("parentId is null for a root-level thread (no parent)", () => {
    const node = makeNode({
      semantic_kind: "thread",
      source_id: "t_root",
      virtual_path: "Chat",
      parent_id: null,
    });
    const result = toSidebarWorkspaceNode(node);
    expect(result.parentId).toBeNull();
  });

  it("parentId propagates to nested children", () => {
    const parent = makeNode({ id: "folder-1", semantic_kind: "folder", kind: "folder", name: "Work" });
    const child = makeNode({
      id: "thread-1",
      semantic_kind: "thread",
      source_id: "t_child",
      virtual_path: "Work/Chat",
      parent_id: "folder-1",
    });
    const parentResult = toSidebarWorkspaceNode({ ...parent, children: [child] });
    const childResult = parentResult.children?.[0];
    expect(childResult?.parentId).toBe("folder-1");
  });
});
