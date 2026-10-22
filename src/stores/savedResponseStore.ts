// Zustand store for saved responses.
// Manages per-request saved response snapshots: save, list, rename, delete.

import { create } from "zustand";
import type { HttpResponse, SavedResponse } from "../types";
import * as api from "../api/savedResponses";

interface SavedResponseState {
  /** Map from requestId → SavedResponse[]. Loaded on demand. */
  byRequest: Record<string, SavedResponse[]>;
  loading: boolean;
  error: string | null;

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  /** Load all saved responses for a request. */
  loadForRequest: (requestId: string) => Promise<void>;

  /**
   * Save a new response snapshot.
   * Auto-generates a label like "Response 1", "Response 2", etc.
   * based on the current count for this request.
   */
  saveResponse: (requestId: string, response: HttpResponse) => Promise<SavedResponse>;

  /** Rename an existing saved response. */
  rename: (id: string, requestId: string, newLabel: string) => Promise<void>;

  /** Delete a saved response. */
  remove: (id: string, requestId: string) => Promise<void>;

  /** Clear cached responses for a request (e.g. when switching requests). */
  clearForRequest: (requestId: string) => void;
}

export const useSavedResponseStore = create<SavedResponseState>((set, get) => ({
  byRequest: {},
  loading: false,
  error: null,

  loadForRequest: async (requestId) => {
    set({ loading: true, error: null });
    try {
      const responses = await api.listSavedResponses(requestId);
      set((s) => ({
        byRequest: { ...s.byRequest, [requestId]: responses },
        loading: false,
      }));
    } catch (err) {
      set({ loading: false, error: String(err) });
    }
  },

  saveResponse: async (requestId, response) => {
    const existing = get().byRequest[requestId] ?? [];
    const label = `Response ${existing.length + 1}`;

    const saved = await api.saveResponse(
      requestId,
      label,
      response.status_code,
      response.status_text,
      response.headers,
      response.body,
      response.response_time_ms,
      response.response_size_bytes
    );

    set((s) => ({
      byRequest: {
        ...s.byRequest,
        [requestId]: [...(s.byRequest[requestId] ?? []), saved],
      },
    }));

    return saved;
  },

  rename: async (id, requestId, newLabel) => {
    await api.renameSavedResponse(id, newLabel);
    set((s) => ({
      byRequest: {
        ...s.byRequest,
        [requestId]: (s.byRequest[requestId] ?? []).map((r) =>
          r.id === id ? { ...r, label: newLabel } : r
        ),
      },
    }));
  },

  remove: async (id, requestId) => {
    await api.deleteSavedResponse(id);
    set((s) => ({
      byRequest: {
        ...s.byRequest,
        [requestId]: (s.byRequest[requestId] ?? []).filter((r) => r.id !== id),
      },
    }));
  },

  clearForRequest: (requestId) => {
    set((s) => {
      const next = { ...s.byRequest };
      delete next[requestId];
      return { byRequest: next };
    });
  },
}));
