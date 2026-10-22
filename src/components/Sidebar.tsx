// Collections sidebar — full implementation (tasks 18 + 21).
//
// 18.1 — Tree view: recursive Collection/Request nodes, expand/collapse,
//         click-to-load request into the builder.
// 18.2 — CRUD UI: context menu for create/rename/delete/duplicate,
//         inline rename input.
// 21.1 — Import button in the sidebar header; opens file dialog, refreshes list.
// 21.2 — Export option in the collection context menu; opens save dialog.
//
// Requirements: 2.1, 2.2, 2.4, 2.6, 6.1, 6.2, 6.3, 6.4, 8.2

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactElement,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { createPortal } from "react-dom";
import { useCollectionStore } from "../stores/collectionStore";
import { useRequestStore } from "../stores/requestStore";
import type { AuthConfig, Collection, HttpMethod, Request } from "../types";
import { DEFAULT_AUTH, DEFAULT_BODY } from "../types";
import { exportCollection, importCollection } from "../api/importExport";
import ConfirmDialog from "./ConfirmDialog";

// ---------------------------------------------------------------------------
// HTTP method badge colours
// ---------------------------------------------------------------------------

const METHOD_COLORS: Record<HttpMethod, string> = {
  GET: "#16a34a",
  POST: "#d97706",
  PUT: "#2563eb",
  PATCH: "#7c3aed",
  DELETE: "#dc2626",
  HEAD: "#6b7280",
  OPTIONS: "#0891b2",
};

// ---------------------------------------------------------------------------
// SVG icon components — stroke-based, consistent with the chevron style
// strokeWidth 1.5, round caps & joins, 16×16 viewport
// ---------------------------------------------------------------------------

const IconFolder = ({ open }: { open?: boolean }) => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    {open ? (
      // open folder: base + lifted top flap
      <path
        d="M1 4.5A1.5 1.5 0 0 1 2.5 3h3.382a1.5 1.5 0 0 1 1.06.44l.618.618A1.5 1.5 0 0 0 8.62 4.5H13.5A1.5 1.5 0 0 1 15 6v6a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12V4.5Z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    ) : (
      // closed folder
      <>
        <path
          d="M1 5.5A1.5 1.5 0 0 1 2.5 4h3.382a1.5 1.5 0 0 1 1.06.44l.618.618A1.5 1.5 0 0 0 8.62 5.5H13.5A1.5 1.5 0 0 1 15 7v5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12V5.5Z"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M1 8h14"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        />
      </>
    )}
  </svg>
);

// Collection icon — stacked layers, distinct from folder
const IconCollection = ({ open }: { open?: boolean }) => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    {/* bottom layer */}
    <rect x="2" y="10" width="12" height="3.5" rx="1" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    {/* middle layer */}
    <rect x="2" y="6" width="12" height="3.5" rx="1" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    {/* top layer — lifts open when expanded */}
    <rect
      x="2" y="2" width="12" height="3.5" rx="1"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinejoin="round"
      style={{ transform: open ? "translateY(-1px)" : "none", transition: "transform 150ms ease" }}
    />
  </svg>
);

// Import: arrow pointing down into a tray
const IconUpload = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M8 3v8M4 8l4 4 4-4"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M2 13h12"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

// Export: arrow pointing up out of a tray
const IconDownload = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M8 11V3M4 7l4-4 4 4"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M2 13h12"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

// Plus sign (new collection / new request)
const IconPlus = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M8 3v10M3 8h10"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

// Horizontal ellipsis (more actions)
const IconEllipsis = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <circle cx="4"  cy="8" r="1.25" fill="currentColor" />
    <circle cx="8"  cy="8" r="1.25" fill="currentColor" />
    <circle cx="12" cy="8" r="1.25" fill="currentColor" />
  </svg>
);

// X / close
const IconClose = () => (
  <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M3 3l10 10M13 3L3 13"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);

// Pencil (rename)
const IconPencil = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M11.5 2.5a1.414 1.414 0 0 1 2 2L5 13H3v-2L11.5 2.5Z"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// Trash (delete)
const IconTrash = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M2 4h12M5 4V2h6v2M6 7v5M10 7v5M3 4l1 9a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1l1-9"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// Arrow diagonal / open
const IconOpen = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M9 3h4v4"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M13 3L7 9"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
    <path
      d="M6 4H4a1 1 0 0 0-1 1v7a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-2"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// Copy / duplicate
const IconDuplicate = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <rect
      x="5" y="5" width="8" height="8" rx="1"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M3 11V3a1 1 0 0 1 1-1h8"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// Lock / auth
const IconLock = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <rect x="3" y="7" width="10" height="7" rx="1.5" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    <path d="M5 7V5a3 3 0 0 1 6 0v2" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <circle cx="8" cy="10.5" r="1" fill="currentColor" />
  </svg>
);

// ---------------------------------------------------------------------------
// Auth type helpers
// ---------------------------------------------------------------------------

const AUTH_TYPE_LABELS: Record<AuthConfig["type"], string> = {
  none: "No Auth",
  inherit: "Inherit",
  basic_auth: "Basic Auth",
  bearer_token: "Bearer Token",
  api_key: "API Key",
};

// ---------------------------------------------------------------------------
// CollectionAuthModal — edit auth for a collection/folder
// ---------------------------------------------------------------------------

interface CollectionAuthModalProps {
  collection: Collection;
  onSave: (auth: AuthConfig) => Promise<void>;
  onClose: () => void;
}

function CollectionAuthModal({ collection, onSave, onClose }: CollectionAuthModalProps) {
  const isFolder = collection.parent_id !== null;
  const [auth, setAuth] = useState<AuthConfig>(collection.auth);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const authTypes: { id: AuthConfig["type"]; label: string }[] = [
    ...(isFolder ? [{ id: "inherit" as const, label: "Inherit" }] : []),
    { id: "none", label: "No Auth" },
    { id: "basic_auth", label: "Basic Auth" },
    { id: "bearer_token", label: "Bearer Token" },
    { id: "api_key", label: "API Key" },
  ];

  const switchType = (newType: AuthConfig["type"]) => {
    if (newType === auth.type) return;
    switch (newType) {
      case "inherit": setAuth({ type: "inherit" }); break;
      case "none": setAuth({ type: "none" }); break;
      case "basic_auth": setAuth({ type: "basic_auth", username: "", password: "" }); break;
      case "bearer_token": setAuth({ type: "bearer_token", token: "" }); break;
      case "api_key": setAuth({ type: "api_key", key: "", value: "", location: "header" }); break;
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await onSave(auth);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={`Edit auth for ${collection.name}`}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="modal-panel" style={{ maxWidth: 520 }}>
        {/* Header */}
        <div className="modal-panel__header">
          <span className="modal-panel__title">
            {isFolder ? "Folder" : "Collection"} Auth — <em>{collection.name}</em>
          </span>
          <button
            className="btn btn--ghost btn--icon"
            aria-label="Close"
            onClick={onClose}
          >
            <IconClose />
          </button>
        </div>

        {/* Body */}
        <div className="modal-panel__body" style={{ padding: "16px 20px 20px" }}>
          {/* Type selector */}
          <div className="auth-editor__type-bar" style={{ marginBottom: 16 }}>
            <label className="form-label" style={{ marginRight: 8, whiteSpace: "nowrap" }}>
              Auth type
            </label>
            <div className="auth-editor__type-btns">
              {authTypes.map(({ id, label }) => (
                <button
                  key={id}
                  className={`btn btn--ghost auth-editor__type-btn${auth.type === id ? " auth-editor__type-btn--active" : ""}`}
                  onClick={() => switchType(id)}
                  aria-pressed={auth.type === id}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {/* Fields */}
          <div className="auth-editor__fields">
            {auth.type === "inherit" && (
              <p className="auth-editor__empty-note">
                Auth will be inherited from the parent folder or collection. If none is set up the chain, no authentication is sent.
              </p>
            )}
            {auth.type === "none" && (
              <p className="auth-editor__empty-note">
                No authentication will be applied to requests in this {isFolder ? "folder" : "collection"}.
              </p>
            )}
            {auth.type === "basic_auth" && (
              <div className="auth-editor__grid">
                <div className="form-group">
                  <label htmlFor="col-auth-username" className="form-label">Username</label>
                  <input id="col-auth-username" className="input" type="text"
                    value={auth.username}
                    onChange={(e) => setAuth({ type: "basic_auth", username: e.target.value, password: auth.password })}
                    placeholder="username" autoComplete="off" />
                </div>
                <div className="form-group">
                  <label htmlFor="col-auth-password" className="form-label">Password</label>
                  <input id="col-auth-password" className="input" type="password"
                    value={auth.password}
                    onChange={(e) => setAuth({ type: "basic_auth", username: auth.username, password: e.target.value })}
                    placeholder="password" autoComplete="new-password" />
                </div>
                <p className="auth-editor__note">
                  Sent as <code>Authorization: Basic …</code> for requests that inherit this auth.
                </p>
              </div>
            )}
            {auth.type === "bearer_token" && (
              <div className="form-group" style={{ maxWidth: 480 }}>
                <label htmlFor="col-auth-token" className="form-label">Token</label>
                <input id="col-auth-token" className="input" type="text"
                  value={auth.token}
                  onChange={(e) => setAuth({ type: "bearer_token", token: e.target.value })}
                  placeholder="your-token-here" autoComplete="off" />
                <p className="auth-editor__note" style={{ marginTop: 6 }}>
                  Sent as <code>Authorization: Bearer &lt;token&gt;</code>.
                </p>
              </div>
            )}
            {auth.type === "api_key" && (
              <div className="auth-editor__grid">
                <div className="form-group">
                  <label htmlFor="col-api-key-name" className="form-label">Key name</label>
                  <input id="col-api-key-name" className="input" type="text"
                    value={auth.key}
                    onChange={(e) => setAuth({ type: "api_key", key: e.target.value, value: auth.value, location: auth.location })}
                    placeholder="X-API-Key" autoComplete="off" />
                </div>
                <div className="form-group">
                  <label htmlFor="col-api-key-value" className="form-label">Value</label>
                  <input id="col-api-key-value" className="input" type="text"
                    value={auth.value}
                    onChange={(e) => setAuth({ type: "api_key", key: auth.key, value: e.target.value, location: auth.location })}
                    placeholder="your-api-key" autoComplete="off" />
                </div>
                <div className="form-group">
                  <label className="form-label">Add to</label>
                  <div className="auth-editor__location-btns">
                    {(["header", "query_param"] as const).map((loc) => (
                      <button key={loc}
                        className={`btn btn--ghost${auth.location === loc ? " auth-editor__type-btn--active" : ""}`}
                        onClick={() => setAuth({ type: "api_key", key: auth.key, value: auth.value, location: loc })}
                        aria-pressed={auth.location === loc}>
                        {loc === "header" ? "Header" : "Query param"}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Error */}
          {error && <p role="alert" className="send-error" style={{ marginTop: 8 }}>{error}</p>}

          {/* Footer actions */}
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
            <button className="btn btn--secondary" onClick={onClose} disabled={saving}>
              Cancel
            </button>
            <button className="btn btn--primary" onClick={handleSave} disabled={saving} aria-busy={saving}>
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Context menu types
// ---------------------------------------------------------------------------

type ContextMenuTarget =
  | { kind: "collection"; collection: Collection }
  | { kind: "request"; request: Request; collectionId: string };

interface ContextMenuState {
  target: ContextMenuTarget;
  x: number;
  y: number;
}

// ---------------------------------------------------------------------------
// Sidebar root
// ---------------------------------------------------------------------------

export default function Sidebar() {
  const {
    collections,
    requestsByCollection,
    loading,
    error,
    loadCollections,
    loadRequests,
    createCollection,
    updateCollection,
    deleteCollection,
    createRequest,
    updateRequest,
    deleteRequest,
    duplicateRequest,
    moveRequest,
  } = useCollectionStore();
  const { loadRequest, draft } = useRequestStore();
  const activeRequestId = draft.id;

  // Track which collection nodes are expanded
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  // Inline rename state
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const renameInputRef = useRef<HTMLInputElement>(null);

  // Context menu state
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);
  const ctxMenuRef = useRef<HTMLDivElement>(null);

  // Auth modal state
  const [authModalCollection, setAuthModalCollection] = useState<Collection | null>(null);

  // Pending delete confirmation
  const [pendingDelete, setPendingDelete] = useState<
    | { kind: "collection"; id: string }
    | { kind: "request"; id: string; collectionId: string }
    | null
  >(null);

  // Import / export feedback state
  const [importPending, setImportPending] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  // ---------------------------------------------------------------------------
  // Drag-and-drop state (pointer-events based — works in all WebViews)
  // ---------------------------------------------------------------------------

  interface DragPayload {
    id: string;
    sourceCollectionId: string;
    label: string;
    method: string;
    methodColor: string;
  }

  const [draggedRequest, setDraggedRequest] = useState<DragPayload | null>(null);
  const draggedRequestRef = useRef<DragPayload | null>(null);

  /** Current cursor position for the drag ghost */
  const [dragPos, setDragPos] = useState({ x: 0, y: 0 });
  const dragPosRef = useRef({ x: 0, y: 0 });

  /** The collection ID currently highlighted as drop target */
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const dropTargetIdRef = useRef<string | null>(null);

  /** True while a drag gesture (≥5px) is in progress — used to suppress click */
  const isDraggingRef = useRef(false);

  /** Sidebar scroll container ref */
  const sidebarRef = useRef<HTMLDivElement>(null);

  // ---------------------------------------------------------------------------
  // Context menu close (declared early so handlers below can reference it)
  // ---------------------------------------------------------------------------

  const closeCtxMenu = useCallback(() => setCtxMenu(null), []);

  // Handler: save collection auth from modal
  const handleSaveCollectionAuth = useCallback(
    async (auth: AuthConfig) => {
      if (!authModalCollection) return;
      await updateCollection(
        authModalCollection.id,
        authModalCollection.name,
        authModalCollection.description,
        auth
      );
    },
    [authModalCollection, updateCollection]
  );

  // ---------------------------------------------------------------------------
  // Pointer-based drag handlers
  // ---------------------------------------------------------------------------

  const startDrag = useCallback(
    (
      e: React.PointerEvent,
      requestId: string,
      sourceCollectionId: string,
      label: string,
      method: string,
      methodColor: string,
    ) => {
      if (e.button !== 0) return;
      // Prevent text selection and default browser drag during pointer drag
      e.preventDefault();

      const originX = e.clientX;
      const originY = e.clientY;
      let dragStarted = false;

      const payload: DragPayload = { id: requestId, sourceCollectionId, label, method, methodColor };

      const onMove = (ev: PointerEvent) => {
        const dx = ev.clientX - originX;
        const dy = ev.clientY - originY;

        // Only start drag after moving 5px (threshold prevents accidental drags)
        if (!dragStarted) {
          if (Math.hypot(dx, dy) < 5) return;
          dragStarted = true;
          isDraggingRef.current = true;
          draggedRequestRef.current = payload;
          setDraggedRequest(payload);
        }

        const pos = { x: ev.clientX, y: ev.clientY };
        dragPosRef.current = pos;
        setDragPos(pos);

        // Hit-test: walk up from element under cursor for data-collection-id
        const el = document.elementFromPoint(ev.clientX, ev.clientY);
        let found: string | null = null;
        if (el) {
          let node: Element | null = el;
          while (node) {
            const cid = node.getAttribute("data-collection-id");
            if (cid) { found = cid; break; }
            node = node.parentElement;
          }
        }
        const next = (found && found !== draggedRequestRef.current?.sourceCollectionId) ? found : null;
        if (next !== dropTargetIdRef.current) {
          dropTargetIdRef.current = next;
          setDropTargetId(next);
        }
      };

      const onUp = async () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);

        if (!dragStarted) {
          // Was a click, not a drag — nothing to do for DnD
          isDraggingRef.current = false;
          return;
        }

        const dragging = draggedRequestRef.current;
        draggedRequestRef.current = null;
        setDraggedRequest(null);
        const hitTarget = dropTargetIdRef.current;
        dropTargetIdRef.current = null;
        setDropTargetId(null);

        // Reset isDraggingRef after a tick so the synthetic click event
        // (which fires after pointerup) sees it as true and is suppressed
        setTimeout(() => { isDraggingRef.current = false; }, 0);

        if (!dragging || !hitTarget || hitTarget === dragging.sourceCollectionId) return;

        await loadRequests(hitTarget);
        try {
          await moveRequest(dragging.id, dragging.sourceCollectionId, hitTarget);
          setExpanded((prev) => new Set([...prev, hitTarget]));
        } catch (err) {
          console.error("[DnD] move error:", err);
        }
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    },
    [moveRequest, loadRequests]
  );

  // onPointerMove / endDrag stubs — movement handled via global window listeners in startDrag

  // ---------------------------------------------------------------------------
  // Import handler (21.1)
  // ---------------------------------------------------------------------------

  const handleImport = useCallback(async () => {
    setImportError(null);
    setImportPending(true);
    try {
      const result = await importCollection();
      if (result !== null) {
        // Refresh the full collection list so the new collection appears.
        await loadCollections();
      }
    } catch (err) {
      setImportError(String(err));
    } finally {
      setImportPending(false);
    }
  }, [loadCollections]);

  // ---------------------------------------------------------------------------
  // Export handler (21.2)
  // ---------------------------------------------------------------------------

  const handleExport = useCallback(
    async (collectionId: string) => {
      setExportError(null);
      closeCtxMenu();
      try {
        await exportCollection(collectionId);
      } catch (err) {
        setExportError(String(err));
      }
    },
    [closeCtxMenu]
  );

  // "New collection" inline input
  const [creatingRoot, setCreatingRoot] = useState(false);
  const [newColName, setNewColName] = useState("");
  const newColInputRef = useRef<HTMLInputElement>(null);

  // Load on mount
  useEffect(() => {
    loadCollections();
  }, [loadCollections]);

  // Auto-focus rename input when it appears
  useEffect(() => {
    if (renamingId && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renamingId]);

  // Auto-focus new collection input
  useEffect(() => {
    if (creatingRoot && newColInputRef.current) {
      newColInputRef.current.focus();
    }
  }, [creatingRoot]);

  // Close context menu on outside click
  useEffect(() => {
    if (!ctxMenu) return;
    const handler = (e: globalThis.MouseEvent) => {
      if (ctxMenuRef.current && !ctxMenuRef.current.contains(e.target as Node)) {
        setCtxMenu(null);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [ctxMenu]);

  // ---------------------------------------------------------------------------
  // Expand / collapse
  // ---------------------------------------------------------------------------

  const toggleExpand = useCallback(
    (id: string) => {
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
          // Lazy-load requests when expanding
          if (!requestsByCollection[id]) {
            loadRequests(id);
          }
        }
        return next;
      });
    },
    [requestsByCollection, loadRequests]
  );

  // ---------------------------------------------------------------------------
  // Context menu handlers
  // ---------------------------------------------------------------------------

  const openCtxMenu = useCallback(
    (e: MouseEvent, target: ContextMenuTarget) => {
      e.preventDefault();
      e.stopPropagation();
      setCtxMenu({ target, x: e.clientX, y: e.clientY });
    },
    []
  );

  // ---------------------------------------------------------------------------
  // Rename helpers
  // ---------------------------------------------------------------------------

  const startRename = useCallback((id: string, currentName: string) => {
    setRenamingId(id);
    setRenameValue(currentName);
    closeCtxMenu();
  }, [closeCtxMenu]);

  const commitRename = useCallback(async () => {
    if (!renamingId) return;
    const trimmed = renameValue.trim();
    if (trimmed) {
      // Determine if this is a collection or a request
      const isCollection = collections.some((c) => c.id === renamingId);
      if (isCollection) {
        const col = collections.find((c) => c.id === renamingId)!;
        await updateCollection(renamingId, trimmed, col.description, col.auth);
      } else {
        // Find the request across all loaded collections
        for (const [, reqs] of Object.entries(requestsByCollection)) {
          const req = reqs.find((r) => r.id === renamingId);
          if (req) {
            await updateRequest(
              req.id,
              trimmed,
              req.method,
              req.url,
              req.headers,
              req.params,
              req.body,
              req.auth
            );
            break;
          }
        }
      }
    }
    setRenamingId(null);
  }, [renamingId, renameValue, collections, requestsByCollection, updateCollection, updateRequest]);

  const cancelRename = useCallback(() => setRenamingId(null), []);

  const handleRenameKey = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") commitRename();
      if (e.key === "Escape") cancelRename();
    },
    [commitRename, cancelRename]
  );

  // ---------------------------------------------------------------------------
  // Create sub-collection inside a parent
  // ---------------------------------------------------------------------------

  const [creatingChildOf, setCreatingChildOf] = useState<string | null>(null);
  const [newChildColName, setNewChildColName] = useState("");
  const newChildInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (creatingChildOf && newChildInputRef.current) {
      newChildInputRef.current.focus();
    }
  }, [creatingChildOf]);

  const startCreateFolder = useCallback((parentId: string) => {
    setCreatingChildOf(parentId);
    setNewChildColName("");
    // Make sure parent is expanded
    setExpanded((prev) => new Set([...prev, parentId]));
    closeCtxMenu();
  }, [closeCtxMenu]);

  const commitCreateFolder = useCallback(async () => {
    if (!creatingChildOf) return;
    const trimmed = newChildColName.trim();
    if (trimmed) {
      const col = await createCollection(trimmed, null, creatingChildOf);
      setExpanded((prev) => new Set([...prev, col.id]));
    }
    setCreatingChildOf(null);
  }, [creatingChildOf, newChildColName, createCollection]);

  const cancelCreateFolder = useCallback(() => setCreatingChildOf(null), []);

  const handleNewChildKey = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") commitCreateFolder();
      if (e.key === "Escape") cancelCreateFolder();
    },
    [commitCreateFolder, cancelCreateFolder]
  );

  // ---------------------------------------------------------------------------
  // Create request inside a collection
  // ---------------------------------------------------------------------------

  const startCreateRequest = useCallback(
    async (collectionId: string) => {
      const req = await createRequest(
        collectionId,
        "New Request",
        "GET",
        "",
        [],
        [],
        DEFAULT_BODY,
        DEFAULT_AUTH
      );
      loadRequest(req);
      // Expand the parent so the new request is visible
      setExpanded((prev) => new Set([...prev, collectionId]));
      closeCtxMenu();
    },
    [createRequest, loadRequest, closeCtxMenu]
  );

  // ---------------------------------------------------------------------------
  // Delete helpers
  // ---------------------------------------------------------------------------

  const handleDeleteCollection = useCallback(
    (id: string) => {
      setPendingDelete({ kind: "collection", id });
    },
    []
  );

  const handleDeleteRequest = useCallback(
    (id: string, collectionId: string) => {
      setPendingDelete({ kind: "request", id, collectionId });
    },
    []
  );

  const confirmDelete = useCallback(async () => {
    const pending = pendingDelete;
    setPendingDelete(null);
    if (!pending) return;
    if (pending.kind === "collection") {
      await deleteCollection(pending.id);
    } else {
      await deleteRequest(pending.id, pending.collectionId);
    }
    closeCtxMenu();
  }, [pendingDelete, deleteCollection, deleteRequest, closeCtxMenu]);

  const handleDuplicateRequest = useCallback(
    async (id: string, collectionId: string) => {
      const copy = await duplicateRequest(id, collectionId);
      loadRequest(copy);
      closeCtxMenu();
    },
    [duplicateRequest, loadRequest, closeCtxMenu]
  );

  // ---------------------------------------------------------------------------
  // Root-level collection creation
  // ---------------------------------------------------------------------------

  const commitCreateRoot = async () => {
    const trimmed = newColName.trim();
    if (trimmed) {
      const col = await createCollection(trimmed, null, null);
      setExpanded((prev) => new Set([...prev, col.id]));
    }
    setCreatingRoot(false);
    setNewColName("");
  };

  const handleNewRootKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") commitCreateRoot();
    if (e.key === "Escape") {
      setCreatingRoot(false);
      setNewColName("");
    }
  };

  // ---------------------------------------------------------------------------
  // Build collection tree (top-level only; children rendered recursively)
  // ---------------------------------------------------------------------------

  const roots = collections.filter((c) => c.parent_id === null);
  const childrenOf = (parentId: string) =>
    collections.filter((c) => c.parent_id === parentId);

  // ---------------------------------------------------------------------------
  // Render a single collection node (recursive)
  // ---------------------------------------------------------------------------

  const renderCollection = (col: Collection, depth: number): ReactElement => {
    const isExpanded = expanded.has(col.id);
    const children = childrenOf(col.id);
    const requests = (requestsByCollection[col.id] ?? []).slice().sort(
      (a, b) => a.position - b.position
    );
    const isRenaming = renamingId === col.id;
    const isCreatingChildHere = creatingChildOf === col.id;
    const isDropTarget = dropTargetId === col.id;

    const indent = depth * 14;

    return (
      <div key={col.id} className="tree-collection">
        {/* Collection header row — data-collection-id enables pointer hit-test */}
        <div
          data-collection-id={col.id}
          className={`tree-row tree-row--collection${isDropTarget ? " tree-row--drop-target" : ""}`}
          style={{ paddingLeft: 8 + indent }}
          onContextMenu={(e) => openCtxMenu(e, { kind: "collection", collection: col })}
        >
          {/* Expand/collapse toggle */}
          <button
            className="tree-row__toggle btn btn--ghost btn--icon"
            aria-label={isExpanded ? "Collapse" : "Expand"}
            aria-expanded={isExpanded}
            onClick={() => toggleExpand(col.id)}
            tabIndex={0}
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              aria-hidden="true"
              style={{
                transform: isExpanded ? "rotate(0deg)" : "rotate(-90deg)",
                transition: "transform 150ms ease",
                display: "block",
              }}
            >
              <path
                d="M1 3l4 4 4-4"
                stroke="currentColor"
                strokeWidth="1.5"
                fill="none"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>

          {/* Collection or folder icon — different shape and colour per depth */}
          <span
            className="tree-row__icon"
            aria-hidden="true"
            style={{ color: depth === 0 ? "#6366f1" : "#f59e0b" }}
          >
            {depth === 0
              ? <IconCollection open={isExpanded} />
              : <IconFolder open={isExpanded} />}
          </span>

          {/* Name — either static label or inline rename input */}
          {isRenaming ? (
            <input
              ref={renameInputRef}
              className="tree-row__rename-input input"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onKeyDown={handleRenameKey}
              onBlur={commitRename}
              aria-label="Rename collection"
            />
          ) : (
            <span
              className="tree-row__name"
              title={col.name}
              onClick={() => toggleExpand(col.id)}
            >
              {col.name}
              {col.auth && col.auth.type !== "none" && col.auth.type !== "inherit" && (
                <span
                  className="tree-row__auth-badge"
                  title={`Auth: ${AUTH_TYPE_LABELS[col.auth.type]}`}
                  aria-label={`Auth: ${AUTH_TYPE_LABELS[col.auth.type]}`}
                >
                  <IconLock />
                </span>
              )}
            </span>
          )}

          {/* Inline action buttons (visible on hover) */}
          {!isRenaming && (
            <div className="tree-row__actions">
              <button
                className="btn btn--ghost btn--icon tree-row__action-btn"
                title="Add request"
                aria-label={`Add request to ${col.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  startCreateRequest(col.id);
                }}
              >
                <IconPlus />
              </button>
              <button
                className="btn btn--ghost btn--icon tree-row__action-btn"
                title="More actions"
                aria-label={`More actions for ${col.name}`}
                onClick={(e) => openCtxMenu(e, { kind: "collection", collection: col })}
              >
                <IconEllipsis />
              </button>
            </div>
          )}
        </div>

        {/* Expanded content */}
        {isExpanded && (
          <div className="tree-collection__children">
            {/* Sub-collections */}
            {children.map((child) => renderCollection(child, depth + 1))}

            {/* Inline "new folder" input */}
            {isCreatingChildHere && (
              <div
                className="tree-row tree-row--new-folder"
                style={{ paddingLeft: 8 + indent + 14 }}
              >
                <span className="tree-row__icon" aria-hidden="true" style={{ color: "#f59e0b" }}>
                  <IconFolder />
                </span>
                <input
                  ref={newChildInputRef}
                  className="tree-row__rename-input input"
                  placeholder="Folder name…"
                  value={newChildColName}
                  onChange={(e) => setNewChildColName(e.target.value)}
                  onKeyDown={handleNewChildKey}
                  onBlur={commitCreateFolder}
                  aria-label="New folder name"
                />
              </div>
            )}

            {/* Requests */}
            {requests.map((req) => renderRequest(req, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  // ---------------------------------------------------------------------------
  // Render a single request leaf node
  // ---------------------------------------------------------------------------

  const renderRequest = (req: Request, depth: number): ReactElement => {
    const indent = depth * 14;
    const methodColor = METHOD_COLORS[req.method] ?? "#6b7280";
    const isRenaming = renamingId === req.id;

    return (
      <div
        key={req.id}
        className={`tree-row tree-row--request${draggedRequest?.id === req.id ? " tree-row--dragging" : ""}${req.id === activeRequestId ? " tree-row--active" : ""}`}
        style={{ paddingLeft: 8 + indent, touchAction: "none" }}
        role="button"
        tabIndex={0}
        aria-label={`${req.method} ${req.name}`}
        onPointerDown={(e) => {
          if (!isRenaming) {
            startDrag(e, req.id, req.collection_id, req.name, req.method, methodColor);
          }
        }}
        onClick={() => {
          // Suppress click if this was a drag gesture
          if (!isRenaming && !isDraggingRef.current) loadRequest(req);
        }}
        onKeyDown={(e) => {
          if (!isRenaming && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            loadRequest(req);
          }
        }}
        onContextMenu={(e) =>
          openCtxMenu(e, { kind: "request", request: req, collectionId: req.collection_id })
        }
      >
        {/* Spacer for toggle column alignment */}
        <span className="tree-row__toggle-spacer" />

        {/* HTTP method badge */}
        <span
          className="tree-row__method-badge"
          style={{ color: methodColor }}
          aria-hidden
        >
          {req.method}
        </span>

        {/* Request name — static or inline rename input */}
        {isRenaming ? (
          <input
            ref={renameInputRef}
            className="tree-row__rename-input input"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onKeyDown={handleRenameKey}
            onBlur={commitRename}
            aria-label="Rename request"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="tree-row__name" title={req.name}>
            {req.name}
          </span>
        )}

        {/* Inline action button */}
        {!isRenaming && (
          <div className="tree-row__actions">
            <button
              className="btn btn--ghost btn--icon tree-row__action-btn"
              title="More actions"
              aria-label={`More actions for ${req.name}`}
              onClick={(e) =>
                openCtxMenu(e, {
                  kind: "request",
                  request: req,
                  collectionId: req.collection_id,
                })
              }
            >
              <IconEllipsis />
            </button>
          </div>
        )}
      </div>
    );
  };

  // ---------------------------------------------------------------------------
  // Context menu rendering
  // ---------------------------------------------------------------------------

  const renderContextMenu = () => {
    if (!ctxMenu) return null;
    const { target, x, y } = ctxMenu;

    return (
      <div
        ref={ctxMenuRef}
        className="ctx-menu"
        style={{ top: y, left: x }}
        role="menu"
        aria-label="Context menu"
        onKeyDown={(e) => e.key === "Escape" && closeCtxMenu()}
      >
        {target.kind === "collection" && (
          <>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => startCreateRequest(target.collection.id)}
            >
              <IconPlus />
              New Request
            </button>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => startCreateFolder(target.collection.id)}
            >
              <IconFolder />
              New Folder
            </button>
            <div className="ctx-menu__divider" />
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => startRename(target.collection.id, target.collection.name)}
            >
              <IconPencil />
              Rename
            </button>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => {
                setAuthModalCollection(target.collection);
                closeCtxMenu();
              }}
            >
              <IconLock />
              Edit Auth
            </button>
            <div className="ctx-menu__divider" />
            {target.collection.parent_id === null && (
              <>
                <button
                  className="ctx-menu__item"
                  role="menuitem"
                  onClick={() => handleExport(target.collection.id)}
                >
                  <IconDownload />
                  Export
                </button>
                <div className="ctx-menu__divider" />
              </>
            )}
            <button
              className="ctx-menu__item ctx-menu__item--danger"
              role="menuitem"
              onClick={() => handleDeleteCollection(target.collection.id)}
            >
              <IconTrash />
              Delete
            </button>
          </>
        )}

        {target.kind === "request" && (
          <>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => loadRequest(target.request)}
            >
              <IconOpen />
              Open
            </button>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() =>
                handleDuplicateRequest(target.request.id, target.collectionId)
              }
            >
              <IconDuplicate />
              Duplicate
            </button>
            <button
              className="ctx-menu__item"
              role="menuitem"
              onClick={() => startRename(target.request.id, target.request.name)}
            >
              <IconPencil />
              Rename
            </button>
            <div className="ctx-menu__divider" />
            <button
              className="ctx-menu__item ctx-menu__item--danger"
              role="menuitem"
              onClick={() =>
                handleDeleteRequest(target.request.id, target.collectionId)
              }
            >
              <IconTrash />
              Delete
            </button>
          </>
        )}
      </div>
    );
  };

  // ---------------------------------------------------------------------------
  // Main render
  // ---------------------------------------------------------------------------

  return (
    <>
    <aside
      className="app-sidebar"
      aria-label="Collections"
      ref={sidebarRef}
      style={{ userSelect: draggedRequest ? "none" : undefined }}
    >
      {/* Header */}
      <div className="app-sidebar__header">
        <span>Collections</span>
        <div className="app-sidebar__header-actions">
          <button
            className="btn btn--ghost btn--icon"
            title="Import collection"
            aria-label="Import collection"
            disabled={importPending}
            onClick={handleImport}
          >
            <IconUpload />
          </button>
          <button
            className="btn btn--ghost btn--icon"
            title="New collection"
            aria-label="New collection"
            onClick={() => {
              setCreatingRoot(true);
              setNewColName("");
            }}
          >
            <IconPlus />
          </button>
        </div>
      </div>

      {/* Import / export error notifications */}
      {importError && (
        <div
          role="alert"
          className="sidebar-notification sidebar-notification--error"
        >
          <span>Import failed: {importError}</span>
          <button
            className="btn btn--ghost btn--icon sidebar-notification__close"
            aria-label="Dismiss import error"
            onClick={() => setImportError(null)}
          >
            <IconClose />
          </button>
        </div>
      )}
      {exportError && (
        <div
          role="alert"
          className="sidebar-notification sidebar-notification--error"
        >
          <span>Export failed: {exportError}</span>
          <button
            className="btn btn--ghost btn--icon sidebar-notification__close"
            aria-label="Dismiss export error"
            onClick={() => setExportError(null)}
          >
            <IconClose />
          </button>
        </div>
      )}

      {/* Tree content */}
      <div className="app-sidebar__content">
        {loading && (
          <p
            style={{
              padding: "12px 16px",
              color: "var(--color-text-muted)",
              fontSize: 13,
            }}
          >
            Loading…
          </p>
        )}

        {error && (
          <p
            role="alert"
            style={{
              padding: "12px 16px",
              color: "var(--color-danger)",
              fontSize: 13,
            }}
          >
            {error}
          </p>
        )}

        {/* Root-level collection nodes */}
        {roots.map((col) => renderCollection(col, 0))}

        {/* Inline "new root collection" input */}
        {creatingRoot && (
          <div className="tree-row tree-row--new-folder" style={{ paddingLeft: 8 }}>
            <span className="tree-row__icon" aria-hidden="true">
              <IconFolder />
            </span>
            <input
              ref={newColInputRef}
              className="tree-row__rename-input input"
              placeholder="Collection name…"
              value={newColName}
              onChange={(e) => setNewColName(e.target.value)}
              onKeyDown={handleNewRootKey}
              onBlur={commitCreateRoot}
              aria-label="New collection name"
            />
          </div>
        )}

        {!loading && !error && collections.length === 0 && !creatingRoot && (
          <div className="sidebar-empty">
            <p>No collections yet.</p>
            <button
              className="btn btn--secondary"
              style={{ marginTop: 8 }}
              onClick={() => {
                setCreatingRoot(true);
                setNewColName("");
              }}
            >
              + New Collection
            </button>
          </div>
        )}
      </div>

      {/* Context menu (portal-style, positioned absolutely on viewport) */}
      {renderContextMenu()}

      {/* Drag ghost — follows cursor, looks like the actual request row */}
      {draggedRequest && (
        <div
          className="drag-ghost"
          style={{
            position: "fixed",
            left: dragPos.x + 14,
            top: dragPos.y - 14,
            pointerEvents: "none",
            zIndex: 9999,
          }}
          aria-hidden="true"
        >
          <span
            className="drag-ghost__method"
            style={{ color: draggedRequest.methodColor }}
          >
            {draggedRequest.method}
          </span>
          <span className="drag-ghost__label">{draggedRequest.label}</span>
        </div>
      )}
    </aside>

    {/* Collection auth modal — rendered in a portal so it escapes the sidebar's overflow:hidden */}
    {authModalCollection && createPortal(
      <CollectionAuthModal
        collection={authModalCollection}
        onSave={handleSaveCollectionAuth}
        onClose={() => setAuthModalCollection(null)}
      />,
      document.body
    )}

    {/* Delete confirmation dialog */}
    {pendingDelete && createPortal(
      <ConfirmDialog
        message={
          pendingDelete.kind === "collection"
            ? "Delete this collection and all its contents?"
            : "Delete this request?"
        }
        title={pendingDelete.kind === "collection" ? "Delete Collection" : "Delete Request"}
        onConfirm={confirmDelete}
        onCancel={() => setPendingDelete(null)}
      />,
      document.body
    )}
    
    </>
  );
}
