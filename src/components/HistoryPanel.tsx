// History panel — task 20.
//
// 20.1 — History list view: method, URL, status code, timestamp, sorted DESC.
// 20.2 — History search (URL/method filter), click-to-load, delete per entry,
//         "Clear All" button.
//
// Requirements: 4.2, 4.3, 4.4, 4.5

import { useCallback, useEffect, useState } from "react";
import ConfirmDialog from "./ConfirmDialog";

// ---------------------------------------------------------------------------
// SVG icon — same stroke style as Sidebar icons (strokeWidth 1.5, round caps)
// ---------------------------------------------------------------------------

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
import { useHistoryStore } from "../stores/historyStore";
import { useRequestStore } from "../stores/requestStore";
import type { HistoryEntry, HttpMethod } from "../types";
import { HTTP_METHODS, statusColorClass, formatBytes } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Format an ISO timestamp into a compact local time string. */
function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  const now = new Date();
  const isToday =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();

  if (isToday) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

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
// Component
// ---------------------------------------------------------------------------

export default function HistoryPanel() {
  const { entries, loading, error, loadHistory, searchHistory, deleteEntry, clearAll } =
    useHistoryStore();
  const { loadHistoryEntry, response } = useRequestStore();

  // Local filter state
  const [urlFilter, setUrlFilter] = useState("");
  const [methodFilter, setMethodFilter] = useState<HttpMethod | "">("");
  const [showClearConfirm, setShowClearConfirm] = useState(false);

  // Debounce search: re-query whenever filters change
  useEffect(() => {
    const id = setTimeout(() => {
      const method = methodFilter === "" ? null : (methodFilter as HttpMethod);
      const url = urlFilter.trim() === "" ? null : urlFilter.trim();

      if (url === null && method === null) {
        loadHistory(null);
      } else {
        searchHistory(url, method);
      }
    }, 250);
    return () => clearTimeout(id);
  }, [urlFilter, methodFilter, loadHistory, searchHistory]);

  // Initial load
  useEffect(() => {
    loadHistory(null);
  }, [loadHistory]);

  // Refresh history whenever a new response arrives (backend persists history on each send)
  useEffect(() => {
    if (!response) return;
    const method = methodFilter === "" ? null : (methodFilter as HttpMethod);
    const url = urlFilter.trim() === "" ? null : urlFilter.trim();
    if (url === null && method === null) {
      loadHistory(null);
    } else {
      searchHistory(url, method);
    }
  }, [response]); // eslint-disable-line react-hooks/exhaustive-deps

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  const handleLoadEntry = useCallback(
    (entry: HistoryEntry) => {
      loadHistoryEntry(entry);
    },
    [loadHistoryEntry]
  );

  const handleDeleteEntry = useCallback(
    async (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      await deleteEntry(id);
    },
    [deleteEntry]
  );

  const handleClearAll = useCallback(() => {
    setShowClearConfirm(true);
  }, []);

  const confirmClearAll = useCallback(async () => {
    setShowClearConfirm(false);
    await clearAll();
  }, [clearAll]);

  // ---------------------------------------------------------------------------
  // Render helpers
  // ---------------------------------------------------------------------------

  const renderEntry = (entry: HistoryEntry) => {
    const methodColor = METHOD_COLORS[entry.request.method] ?? "#6b7280";
    const statusClass = statusColorClass(entry.status_code);

    return (
      <div
        key={entry.id}
        className="history-entry"
        role="button"
        tabIndex={0}
        aria-label={`${entry.request.method} ${entry.request.url} — ${entry.status_code}`}
        onClick={() => handleLoadEntry(entry)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            handleLoadEntry(entry);
          }
        }}
      >
        {/* Left: method badge + URL */}
        <div className="history-entry__main">
          <span
            className="history-entry__method"
            style={{ color: methodColor }}
            aria-label={`Method: ${entry.request.method}`}
          >
            {entry.request.method}
          </span>
          <span className="history-entry__url" title={entry.request.url}>
            {entry.request.url || <em className="history-entry__url--empty">No URL</em>}
          </span>
        </div>

        {/* Right: status + meta + delete */}
        <div className="history-entry__meta">
          <span className={`history-entry__status ${statusClass}`}>
            {entry.status_code}
          </span>
          <span className="history-entry__time-ms" title="Response time">
            {entry.response_time_ms} ms
          </span>
          <span className="history-entry__size" title="Response size">
            {formatBytes(entry.response_size_bytes)}
          </span>
          <span className="history-entry__timestamp" title={entry.executed_at}>
            {formatTimestamp(entry.executed_at)}
          </span>
          <button
            className="btn btn--ghost btn--icon history-entry__delete-btn"
            title="Delete this entry"
            aria-label="Delete history entry"
            onClick={(e) => handleDeleteEntry(e, entry.id)}
          >
            ✕
          </button>
        </div>
      </div>
    );
  };

  // ---------------------------------------------------------------------------
  // Main render
  // ---------------------------------------------------------------------------

  return (
    <>
    <div className="history-panel">
      {/* Header */}
      <div className="history-panel__header">
        <span className="history-panel__title">History</span>
        <button
          className="btn btn--ghost btn--icon history-panel__clear-btn"
          title="Clear all history"
          aria-label="Clear all history"
          disabled={entries.length === 0}
          onClick={handleClearAll}
        >
          <IconTrash />
        </button>
      </div>

      {/* Search bar */}
      <div className="history-panel__search">
        <input
          className="input history-panel__url-input"
          type="search"
          placeholder="Filter by URL…"
          value={urlFilter}
          onChange={(e) => setUrlFilter(e.target.value)}
          aria-label="Filter history by URL"
        />
        <select
          className="input history-panel__method-select"
          value={methodFilter}
          onChange={(e) => setMethodFilter(e.target.value as HttpMethod | "")}
          aria-label="Filter history by method"
        >
          <option value="">All methods</option>
          {HTTP_METHODS.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>

      {/* List */}
      <div className="history-panel__list" role="list" aria-label="Request history">
        {loading && (
          <p className="history-panel__status">Loading…</p>
        )}

        {!loading && error && (
          <p className="history-panel__status history-panel__status--error" role="alert">
            {error}
          </p>
        )}

        {!loading && !error && entries.length === 0 && (
          <div className="history-panel__empty">
            <span className="history-panel__empty-icon" aria-hidden>🕐</span>
            <p>No history yet.</p>
            <p className="history-panel__empty-hint">
              Sent requests will appear here.
            </p>
          </div>
        )}

        {entries.map((entry) => (
          <div key={entry.id} role="listitem">
            {renderEntry(entry)}
          </div>
        ))}
      </div>
    </div>

    {showClearConfirm && (
      <ConfirmDialog
        message="Clear all history? This cannot be undone."
        title="Clear History"
        confirmLabel="Clear"
        onConfirm={confirmClearAll}
        onCancel={() => setShowClearConfirm(false)}
      />
    )}
    </>
  );
}
