// Zustand store for Collections and Requests state.
//
// Requirements: 2.1, 2.2, 2.4, 8.2

import { create } from "zustand";
import type {
  AuthConfig,
  Collection,
  Header,
  HttpMethod,
  QueryParam,
  Request,
  RequestBody,
} from "../types";
import { DEFAULT_COLLECTION_AUTH } from "../types";
import * as api from "../api/collections";

interface CollectionState {
  /** Flat list of all collections (top-level + nested folders). */
  collections: Collection[];
  /** Map from collectionId → Request[]. Loaded on demand. */
  requestsByCollection: Record<string, Request[]>;
  /** Loading / error state. */
  loading: boolean;
  error: string | null;

  // ---------------------------------------------------------------------------
  // Actions — Collections
  // ---------------------------------------------------------------------------

  loadCollections: () => Promise<void>;

  createCollection: (
    name: string,
    description?: string | null,
    parentId?: string | null,
    auth?: AuthConfig | null
  ) => Promise<Collection>;

  updateCollection: (
    id: string,
    name: string,
    description?: string | null,
    auth?: AuthConfig | null
  ) => Promise<void>;

  deleteCollection: (id: string) => Promise<void>;

  // ---------------------------------------------------------------------------
  // Actions — Requests
  // ---------------------------------------------------------------------------

  loadRequests: (collectionId: string) => Promise<void>;

  createRequest: (
    collectionId: string,
    name: string,
    method: HttpMethod,
    url: string,
    headers: Header[],
    params: QueryParam[],
    body: RequestBody,
    auth: AuthConfig
  ) => Promise<Request>;

  updateRequest: (
    id: string,
    name: string,
    method: HttpMethod,
    url: string,
    headers: Header[],
    params: QueryParam[],
    body: RequestBody,
    auth: AuthConfig
  ) => Promise<void>;

  deleteRequest: (id: string, collectionId: string) => Promise<void>;

  duplicateRequest: (id: string, collectionId: string) => Promise<Request>;

  reorderRequest: (
    id: string,
    targetPosition: number,
    collectionId: string
  ) => Promise<void>;

  moveRequest: (
    id: string,
    sourceCollectionId: string,
    targetCollectionId: string
  ) => Promise<void>;
}

export const useCollectionStore = create<CollectionState>((set, get) => ({
  collections: [],
  requestsByCollection: {},
  loading: false,
  error: null,

  // ---------------------------------------------------------------------------
  // Collections
  // ---------------------------------------------------------------------------

  loadCollections: async () => {
    set({ loading: true, error: null });
    try {
      const collections = await api.listCollections();
      set({ collections, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  createCollection: async (name, description, parentId, auth) => {
    const collection = await api.createCollection(name, description, parentId, auth ?? DEFAULT_COLLECTION_AUTH);
    set((s) => ({ collections: [...s.collections, collection] }));
    return collection;
  },

  updateCollection: async (id, name, description, auth) => {
    const col = get().collections.find((c) => c.id === id);
    const updated = await api.updateCollection(id, name, description, auth ?? col?.auth ?? DEFAULT_COLLECTION_AUTH);
    set((s) => ({
      collections: s.collections.map((c) => (c.id === id ? updated : c)),
    }));
  },

  deleteCollection: async (id) => {
    await api.deleteCollection(id);
    // Remove the collection and all its descendants from local state.
    set((s) => {
      const descendants = collectDescendantIds(s.collections, id);
      const removedIds = new Set([id, ...descendants]);
      const filtered = s.collections.filter((c) => !removedIds.has(c.id));
      const reqMap = { ...s.requestsByCollection };
      removedIds.forEach((rid) => delete reqMap[rid]);
      return { collections: filtered, requestsByCollection: reqMap };
    });
  },

  // ---------------------------------------------------------------------------
  // Requests
  // ---------------------------------------------------------------------------

  loadRequests: async (collectionId) => {
    try {
      const requests = await api.listRequests(collectionId);
      set((s) => ({
        requestsByCollection: {
          ...s.requestsByCollection,
          [collectionId]: requests,
        },
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  createRequest: async (collectionId, name, method, url, headers, params, body, auth) => {
    const request = await api.createRequest(
      collectionId,
      name,
      method,
      url,
      headers,
      params,
      body,
      auth
    );
    set((s) => {
      const existing = s.requestsByCollection[collectionId] ?? [];
      return {
        requestsByCollection: {
          ...s.requestsByCollection,
          [collectionId]: [...existing, request],
        },
      };
    });
    return request;
  },

  updateRequest: async (id, name, method, url, headers, params, body, auth) => {
    const updated = await api.updateRequest(id, name, method, url, headers, params, body, auth);
    set((s) => {
      const map = { ...s.requestsByCollection };
      const collectionId = updated.collection_id;
      if (map[collectionId]) {
        map[collectionId] = map[collectionId].map((r) =>
          r.id === id ? updated : r
        );
      }
      return { requestsByCollection: map };
    });
  },

  deleteRequest: async (id, collectionId) => {
    await api.deleteRequest(id);
    set((s) => {
      const existing = s.requestsByCollection[collectionId] ?? [];
      return {
        requestsByCollection: {
          ...s.requestsByCollection,
          [collectionId]: existing.filter((r) => r.id !== id),
        },
      };
    });
  },

  duplicateRequest: async (id, collectionId) => {
    const copy = await api.duplicateRequest(id);
    set((s) => {
      const existing = s.requestsByCollection[collectionId] ?? [];
      return {
        requestsByCollection: {
          ...s.requestsByCollection,
          [collectionId]: [...existing, copy],
        },
      };
    });
    return copy;
  },

  reorderRequest: async (id, targetPosition, collectionId) => {
    await api.reorderRequest(id, targetPosition);
    // Reload to get server-authoritative positions.
    await get().loadRequests(collectionId);
  },

  moveRequest: async (id, sourceCollectionId, targetCollectionId) => {
    const moved = await api.moveRequest(id, targetCollectionId);
    set((s) => {
      // Remove from source collection
      const sourceList = (s.requestsByCollection[sourceCollectionId] ?? []).filter(
        (r) => r.id !== id
      );
      // Append to target collection (if already loaded)
      const targetList = s.requestsByCollection[targetCollectionId]
        ? [...s.requestsByCollection[targetCollectionId], moved]
        : [moved];
      return {
        requestsByCollection: {
          ...s.requestsByCollection,
          [sourceCollectionId]: sourceList,
          [targetCollectionId]: targetList,
        },
      };
    });
  },
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Collect all descendant IDs of a collection (recursive). */
function collectDescendantIds(
  collections: Collection[],
  parentId: string
): string[] {
  const children = collections.filter((c) => c.parent_id === parentId);
  return children.flatMap((c) => [c.id, ...collectDescendantIds(collections, c.id)]);
}
