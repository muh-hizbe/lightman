// Tauri invoke wrappers for sample request commands.
// Mirrors commands/sample.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type { Request } from "../types";

/** Return the built-in list of sample requests. */
export function listSamples(): Promise<Request[]> {
  return invoke("list_samples");
}
