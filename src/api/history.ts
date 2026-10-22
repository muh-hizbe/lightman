// Tauri invoke wrappers for history commands.
// Mirrors commands/history.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry, HttpMethod } from "../types";

export function listHistoryEntries(limit?: number | null): Promise<HistoryEntry[]> {
  return invoke("list_history_entries", { limit: limit ?? null });
}

export function searchHistoryEntries(
  urlFilter?: string | null,
  methodFilter?: HttpMethod | null
): Promise<HistoryEntry[]> {
  return invoke("search_history_entries", {
    urlFilter: urlFilter ?? null,
    methodFilter: methodFilter ?? null,
  });
}

export function deleteHistoryEntry(id: string): Promise<void> {
  return invoke("delete_history_entry", { id });
}

export function clearHistory(): Promise<void> {
  return invoke("clear_history");
}
