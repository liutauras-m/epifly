export interface ApiError { status: number; message: string; }
export type ApiResult<T> = { data: T; error: null } | { data: null; error: ApiError };

export interface WorkspaceContent { content: string; }
export interface UploadResponse { id: string; filename: string; size: number; content_type: string; download_url: string; }

export interface InvoiceData {
  invoice_number?: unknown; status?: string; invoice_date?: string; due_date?: string;
  issuer_name?: unknown; issuer_address?: string;
  billed_to_name?: unknown; billed_to_company?: string;
  currency?: string; subtotal?: number; tax_amount?: number; total_amount?: number;
  line_items?: { description?: string; quantity?: unknown; unit_price?: unknown; total?: unknown }[];
}

export type ChatStreamDelta =
  | { kind: 'text'; content: string }
  | { kind: 'tool_start'; id: string; name: string }
  | { kind: 'tool_result'; tool_use_id: string; result: string }
  | { kind: 'thread_id'; id: string }
  | { kind: 'done' };
