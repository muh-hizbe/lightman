// Zustand store for the active request being built/sent, and its response.
//
// Requirements: 1.1, 1.3, 8.2

import { create } from "zustand";
import type {
  AuthConfig,
  Header,
  HistoryEntry,
  HttpMethod,
  HttpResponse,
  QueryParam,
  Request,
  RequestBody,
} from "../types";
import { DEFAULT_AUTH, DEFAULT_BODY } from "../types";
import { sendRequest, cancelRequest } from "../api/http";

// ---------------------------------------------------------------------------
// Path param helpers (frontend-only — substituted into URL before send)
// ---------------------------------------------------------------------------

/** Extract `:paramName` tokens from a URL path segment. */
export function extractPathParams(url: string): string[] {
  const matches = url.match(/:([a-zA-Z_][a-zA-Z0-9_]*)/g) ?? [];
  return [...new Set(matches.map((m) => m.slice(1)))];
}

/** Replace `:paramName` tokens in a URL with values from pathParams array. */
export function substitutePathParams(
  url: string,
  pathParams: { name: string; value: string }[]
): string {
  const map = new Map(pathParams.map((p) => [p.name, p.value]));
  return url.replace(/:([a-zA-Z_][a-zA-Z0-9_]*)/g, (_, name: string) =>
    map.get(name) ?? `:${name}`
  );
}

/** Parse a URL's query string into QueryParam rows (preserving existing enabled state). */
export function parseQueryString(
  rawUrl: string,
  existing: { key: string; value: string; enabled: boolean }[]
): { key: string; value: string; enabled: boolean }[] {
  // Strip everything before "?" — handle {{var}} placeholders safely
  const qIdx = rawUrl.indexOf("?");
  if (qIdx === -1) return [];
  const qs = rawUrl.slice(qIdx + 1);
  const pairs: { key: string; value: string; enabled: boolean }[] = [];
  for (const part of qs.split("&")) {
    if (!part) continue;
    const eqIdx = part.indexOf("=");
    const key = eqIdx === -1 ? part : part.slice(0, eqIdx);
    const value = eqIdx === -1 ? "" : part.slice(eqIdx + 1);
    if (!key) continue;
    const prev = existing.find((e) => e.key === key);
    pairs.push({ key, value, enabled: prev?.enabled ?? true });
  }
  return pairs;
}

/** Rebuild the query string part of a URL from a QueryParam array. */
export function buildQueryString(params: { key: string; value: string; enabled: boolean }[]): string {
  const parts = params
    .filter((p) => p.enabled && p.key.trim())
    .map((p) => (p.value ? `${p.key}=${p.value}` : p.key));
  return parts.length ? `?${parts.join("&")}` : "";
}

/** Replace the query string portion of a URL, keeping the base + path intact. */
export function replaceQueryString(baseUrl: string, params: { key: string; value: string; enabled: boolean }[]): string {
  const qIdx = baseUrl.indexOf("?");
  const base = qIdx === -1 ? baseUrl : baseUrl.slice(0, qIdx);
  return base + buildQueryString(params);
}

export interface PathParam {
  name: string;   // param name without leading ":"
  value: string;
}

/** A transient, editor-facing copy of the request being composed. */
export interface DraftRequest {
  /** Undefined when the request hasn't been saved to a collection yet. */
  id?: string;
  collection_id: string;
  name: string;
  method: HttpMethod;
  url: string;
  headers: Header[];
  params: QueryParam[];
  pathParams: PathParam[];
  body: RequestBody;
  auth: AuthConfig;
}

interface RequestState {
  /** The request currently open in the builder panel. */
  draft: DraftRequest;
  /** The last HTTP response received, or null before any send. */
  response: HttpResponse | null;
  /** True while a request is in-flight. */
  sending: boolean;
  /** Error message from the last send attempt, if any. */
  sendError: string | null;
  /**
   * In-memory cache of responses keyed by request id.
   * Persists responses across navigation for the lifetime of the app session.
   */
  responseCache: Record<string, HttpResponse>;

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  /** Load a saved request into the draft editor. */
  loadRequest: (request: Request) => void;

  /** Load a history entry — restores both the request and its saved response. */
  loadHistoryEntry: (entry: HistoryEntry) => void;

  /** Reset the draft to a blank GET request in the given collection. */
  newRequest: (collectionId: string) => void;

  /** Patch any field(s) on the draft. */
  updateDraft: (patch: Partial<DraftRequest>) => void;

  /** Execute the draft request against the Rust backend. */
  sendDraftRequest: (environmentId: string | null) => Promise<void>;

  /** Cancel the currently in-flight request. */
  cancelDraftRequest: () => Promise<void>;

  /** Clear the last response and any error. */
  clearResponse: () => void;
}

const BLANK_COLLECTION_ID = "";

function makeDraft(collectionId: string): DraftRequest {
  return {
    collection_id: collectionId,
    name: "New Request",
    method: "GET",
    url: "",
    headers: [],
    params: [],
    pathParams: [],
    body: DEFAULT_BODY,
    auth: DEFAULT_AUTH,
  };
}

export const useRequestStore = create<RequestState>((set, get) => ({
  draft: makeDraft(BLANK_COLLECTION_ID),
  response: null,
  sending: false,
  sendError: null,
  responseCache: {},

  loadRequest: (request) => {
    const pathParamNames = extractPathParams(request.url);
    // Restore cached response for this request, if any
    const cachedResponse = request.id ? (get().responseCache[request.id] ?? null) : null;
    set({
      draft: {
        id: request.id,
        collection_id: request.collection_id,
        name: request.name,
        method: request.method,
        url: request.url,
        headers: request.headers,
        params: request.params,
        pathParams: pathParamNames.map((name) => ({ name, value: "" })),
        body: request.body,
        auth: request.auth,
      },
      response: cachedResponse,
      sendError: null,
    });
  },

  loadHistoryEntry: (entry) => {
    const pathParamNames = extractPathParams(entry.request.url);

    // Reconstruct the HttpResponse from the history entry so the user sees
    // the saved response alongside the request.
    const historyResponse: HttpResponse = {
      status_code: entry.status_code,
      status_text: entry.status_text,
      headers: entry.response_headers,
      body: entry.response_body,
      response_time_ms: entry.response_time_ms,
      response_size_bytes: entry.response_size_bytes,
    };

    // Cache the reconstructed response under the request id so it persists
    // across navigation just like a freshly-sent response.
    const requestId = entry.request.id;
    set((s) => ({
      draft: {
        id: entry.request.id,
        collection_id: entry.request.collection_id,
        name: entry.request.name,
        method: entry.request.method,
        url: entry.request.url,
        headers: entry.request.headers,
        params: entry.request.params,
        pathParams: pathParamNames.map((name) => ({ name, value: "" })),
        body: entry.request.body,
        auth: entry.request.auth,
      },
      response: historyResponse,
      sendError: null,
      responseCache: requestId
        ? { ...s.responseCache, [requestId]: historyResponse }
        : s.responseCache,
    }));
  },

  newRequest: (collectionId) => {
    set({
      draft: makeDraft(collectionId),
      response: null,
      sendError: null,
    });
  },

  updateDraft: (patch) => {
    set((s) => ({ draft: { ...s.draft, ...patch } }));
  },

  sendDraftRequest: async (environmentId) => {
    const { draft } = get();

    // Substitute path params into URL before sending
    const resolvedUrl = substitutePathParams(draft.url, draft.pathParams);

    // Build the minimal Request shape required by the Rust backend.
    // Dates are placeholders; the backend only uses them for display in history.
    const now = new Date().toISOString();
    const request: Request = {
      id: draft.id ?? "00000000-0000-0000-0000-000000000000",
      collection_id: draft.collection_id,
      name: draft.name,
      method: draft.method,
      url: resolvedUrl,
      headers: draft.headers,
      params: draft.params,
      body: draft.body,
      auth: draft.auth,
      position: 0,
      created_at: now,
      updated_at: now,
    };

    set({ sending: true, sendError: null });
    try {
      const response = await sendRequest(request, environmentId);
      // Store response in cache keyed by request id so it persists across navigation
      const requestId = draft.id;
      set((s) => ({
        response,
        sending: false,
        responseCache: requestId
          ? { ...s.responseCache, [requestId]: response }
          : s.responseCache,
      }));
    } catch (err) {
      set({ sendError: String(err), sending: false });
    }
  },

  clearResponse: () => {
    const { draft } = get();
    set((s) => ({
      response: null,
      sendError: null,
      responseCache: draft.id
        ? Object.fromEntries(Object.entries(s.responseCache).filter(([k]) => k !== draft.id))
        : s.responseCache,
    }));
  },

  cancelDraftRequest: async () => {
    try {
      await cancelRequest();
    } catch {
      // Ignore errors — if nothing was in-flight, the backend is a no-op.
    }
    // Immediately clear the sending flag so the UI reacts without waiting
    // for the send promise to reject.
    set({ sending: false, sendError: null });
  },
}));
