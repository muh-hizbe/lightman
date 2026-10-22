// Zustand store for Request History state.
//
// Requirements: 4.1, 4.2, 4.3, 4.5, 8.2

import { create } from "zustand";
import type { HistoryEntry, HttpMethod } from "../types";
import * as api from "../api/history";

interface HistoryState {
  entries: HistoryEntry[];
  loading: boolean;
  error: string | null;

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  loadHistory: (limit?: number | null) => Promise<void>;
  searchHistory: (
    urlFilter?: string | null,
    methodFilter?: HttpMethod | null
  ) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  clearAll: () => Promise<void>;
}

export const useHistoryStore = create<HistoryState>((set) => ({
  entries: [],
  loading: false,
  error: null,

  loadHistory: async (limit) => {
    set({ loading: true, error: null });
    try {
      const entries = await api.listHistoryEntries(limit);
      set({ entries, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  searchHistory: async (urlFilter, methodFilter) => {
    set({ loading: true, error: null });
    try {
      const entries = await api.searchHistoryEntries(urlFilter, methodFilter);
      set({ entries, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  deleteEntry: async (id) => {
    await api.deleteHistoryEntry(id);
    set((s) => ({ entries: s.entries.filter((e) => e.id !== id) }));
  },

  clearAll: async () => {
    await api.clearHistory();
    set({ entries: [] });
  },
}));
