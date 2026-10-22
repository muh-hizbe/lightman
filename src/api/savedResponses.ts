// Tauri invoke wrappers for saved response commands.
// Mirrors commands/saved_response.rs

import { invoke } from "@tauri-apps/api/core";
import type { Header, SavedResponse } from "../types";

export function saveResponse(
  requestId: string,
  label: string,
  statusCode: number,
  statusText: string,
  headers: Header[],
  body: string,
  responseTimeMs: number,
  responseSizeBytes: number
): Promise<SavedResponse> {
  return invoke("save_response", {
    requestId,
    label,
    statusCode,
    statusText,
    headers,
    body,
    responseTimeMs,
    responseSizeBytes,
  });
}

export function listSavedResponses(requestId: string): Promise<SavedResponse[]> {
  return invoke("list_saved_responses", { requestId });
}

export function renameSavedResponse(id: string, newLabel: string): Promise<void> {
  return invoke("rename_saved_response", { id, newLabel });
}

export function deleteSavedResponse(id: string): Promise<void> {
  return invoke("delete_saved_response", { id });
}
