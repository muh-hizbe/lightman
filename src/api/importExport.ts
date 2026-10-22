// Tauri invoke wrappers for import/export commands.
// Mirrors commands/export.rs and commands/import.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type { Collection } from "../types";

/**
 * Export a collection to a user-chosen JSON file.
 * Returns the file path written, or null if the user cancelled the dialog.
 */
export function exportCollection(collectionId: string): Promise<string | null> {
  return invoke("export_collection", { collectionId });
}

/**
 * Open a file-picker dialog and import a Postman v2.1 or Lightman JSON file.
 * Returns the newly created Collection, or null if the user cancelled.
 */
export function importCollection(): Promise<Collection | null> {
  return invoke("import_collection");
}
