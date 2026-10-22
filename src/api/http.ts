// Tauri invoke wrappers for HTTP execution command.
// Mirrors commands/http.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type { HttpResponse, Request } from "../types";

/**
 * Send an HTTP request through the full Rust pipeline:
 * variable interpolation → header merge → HTTP execution → history save.
 */
export function sendRequest(
  request: Request,
  environmentId: string | null
): Promise<HttpResponse> {
  return invoke("send_request", { request, environmentId });
}
