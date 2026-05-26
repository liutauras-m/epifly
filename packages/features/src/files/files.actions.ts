import type { ConusSdk } from "@conusai/sdk";
import type { ApiResult, UploadResponse } from "@conusai/sdk";
import type { FileToken } from "@conusai/types";

/**
 * Upload a file into the workspace node tree via the UI upload endpoint.
 * Use for workspace attachments and documents.
 */
export async function uploadWorkspaceFile(
  sdk: ConusSdk,
  file: File
): Promise<ApiResult<UploadResponse>> {
  return sdk.ui.upload(file);
}

/**
 * Upload a file as a chat attachment via the UI upload endpoint.
 * Use for temporary per-message attachments.
 */
export async function uploadUiAttachment(
  sdk: ConusSdk,
  file: File
): Promise<ApiResult<UploadResponse>> {
  return sdk.ui.upload(file);
}

/**
 * Upload a file to the persistent file store via /v1/files.
 * Use for files that outlive a single chat session.
 */
export async function uploadPersistentFile(
  sdk: ConusSdk,
  file: File
): Promise<ApiResult<FileToken>> {
  return sdk.files.upload(file);
}

/**
 * Extract structured invoice data from a previously uploaded file.
 */
export async function extractInvoice(
  sdk: ConusSdk,
  fileId: string
) {
  return sdk.ui.extractInvoice(fileId);
}
