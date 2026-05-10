// Domain types mirroring crates/common Rust structs.
// SessionTrace / UserStep mirror common/src/trace.rs.
// CapabilityCard mirrors agent-core/src/capabilities/card.rs.
// WorkspaceNode mirrors agent-gateway workspace routes.

export type ToolKind = "mcp" | "wasm" | "llm_chain" | "remote_mcp" | "builtin";

export interface CapabilityCard {
  capability_id: string;
  name: string;
  description: string;
  kind: ToolKind;
  tenant_scope: string[];
  tags: string[];
}

export interface UserStep {
  seq: number;
  kind: "click" | "input" | "submit" | "navigate" | "scroll";
  selector: string | null;
  value: string | null;
  url: string;
  timestamp_ms: number;
  screenshot_base64: string | null;
}

export interface SessionTrace {
  id: string;
  started_at: string;
  ended_at: string | null;
  steps: UserStep[];
  urls: string[];
}

export interface WorkspaceNode {
  id: string;
  parent_id: string | null;
  kind: "folder" | "conversation" | "file" | "artifact";
  name: string;
  virtual_path: string;
  last_modified: string;
  created_at?: string;
  updated_at?: string;
  metadata?: { thread_id?: string | null } & Record<string, unknown>;
}

export interface ControlMessage {
  kind: "Heartbeat" | "Replay" | "Stop" | "Ack";
  payload: unknown;
}

export interface FileToken {
  token: string;
  name: string;
  mime_type: string;
  size_bytes: number;
}
