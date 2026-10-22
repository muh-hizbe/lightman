// Domain types — mirrors Rust models in src-tauri/src/models/mod.rs.
// Keep these in sync with the Rust structs and their serde serialisation.
//
// Requirements: 8.2, 8.3

// ---------------------------------------------------------------------------
// HTTP Method
// ---------------------------------------------------------------------------

export type HttpMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "PATCH"
  | "DELETE"
  | "HEAD"
  | "OPTIONS";

export const HTTP_METHODS: HttpMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

// ---------------------------------------------------------------------------
// Header and Query Parameter
// ---------------------------------------------------------------------------

export interface Header {
  key: string;
  value: string;
  enabled: boolean;
}

export interface QueryParam {
  key: string;
  value: string;
  enabled: boolean;
}

// ---------------------------------------------------------------------------
// Request Body
// ---------------------------------------------------------------------------

export interface MultipartField {
  key: string;
  value: string;
  enabled: boolean;
}

export type RequestBody =
  | { type: "none" }
  | { type: "json"; content: string }
  | { type: "form_url_encoded"; fields: MultipartField[] }
  | { type: "multipart"; fields: MultipartField[] }
  | { type: "raw"; content: string; content_type: string }
  | { type: "binary"; file_path: string };

// ---------------------------------------------------------------------------
// Authentication Configuration
// ---------------------------------------------------------------------------

export type ApiKeyLocation = "header" | "query_param";

export type AuthConfig =
  | { type: "none" }
  | { type: "inherit" }
  | { type: "basic_auth"; username: string; password: string }
  | { type: "bearer_token"; token: string }
  | {
      type: "api_key";
      key: string;
      value: string;
      location: ApiKeyLocation;
    };

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

export interface Collection {
  id: string;
  name: string;
  description: string | null;
  parent_id: string | null;
  position: number;
  created_at: string;
  updated_at: string;
  default_headers: Header[];
  /** Auth config for this collection/folder. Requests with `type: "inherit"` resolve from here. */
  auth: AuthConfig;
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

export interface Request {
  id: string;
  collection_id: string;
  name: string;
  method: HttpMethod;
  url: string;
  headers: Header[];
  params: QueryParam[];
  body: RequestBody;
  auth: AuthConfig;
  position: number;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Environment and Variable
// ---------------------------------------------------------------------------

export interface Environment {
  id: string;
  name: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface Variable {
  id: string;
  environment_id: string | null;
  key: string;
  value: string;
  enabled: boolean;
}

// ---------------------------------------------------------------------------
// HTTP Response
// ---------------------------------------------------------------------------

export interface HttpResponse {
  status_code: number;
  status_text: string;
  headers: Header[];
  body: string;
  response_time_ms: number;
  response_size_bytes: number;
}

// ---------------------------------------------------------------------------
// History Entry
// ---------------------------------------------------------------------------

export interface HistoryEntry {
  id: string;
  request: Request;
  status_code: number;
  status_text: string;
  response_time_ms: number;
  response_size_bytes: number;
  response_headers: Header[];
  response_body: string;
  executed_at: string;
}

// ---------------------------------------------------------------------------
// Saved Response
// ---------------------------------------------------------------------------

export interface SavedResponse {
  id: string;
  request_id: string;
  label: string;
  status_code: number;
  status_text: string;
  headers: Header[];
  body: string;
  response_time_ms: number;
  response_size_bytes: number;
  saved_at: string;
}

// ---------------------------------------------------------------------------
// Utility types
// ---------------------------------------------------------------------------

/** Default empty request body. */
export const DEFAULT_BODY: RequestBody = { type: "none" };

/** Default auth for new requests — inherit from parent collection/folder. */
export const DEFAULT_AUTH: AuthConfig = { type: "inherit" };

/** Default auth for new top-level collections — no auth. */
export const DEFAULT_COLLECTION_AUTH: AuthConfig = { type: "none" };

/** Create a new empty Header. */
export function makeHeader(key = "", value = ""): Header {
  return { key, value, enabled: true };
}

/** Create a new empty QueryParam. */
export function makeQueryParam(key = "", value = ""): QueryParam {
  return { key, value, enabled: true };
}

/** Create a new empty MultipartField. */
export function makeMultipartField(key = "", value = ""): MultipartField {
  return { key, value, enabled: true };
}

/** Return a colour class name suitable for displaying an HTTP status code. */
export function statusColorClass(code: number): string {
  if (code >= 500) return "status-5xx";
  if (code >= 400) return "status-4xx";
  if (code >= 300) return "status-3xx";
  if (code >= 200) return "status-2xx";
  return "status-unknown";
}

/** Format a byte count into a human-readable string (B / KB / MB). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
