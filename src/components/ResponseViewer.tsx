// Response viewer panel — includes save response feature.
//
// Sub-tasks:
//   17.1 – Response status bar (status code w/ colour coding, time, size)
//   17.2 – Response body viewer (Pretty / Raw tabs, JSON syntax highlighting)
//   17.3 – Response headers viewer (key-value table)
//   NEW  – Saved responses: save current response, list, rename, delete
//
// Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 8.2

import { useState, useCallback, createContext, useContext, useEffect, useRef } from "react";
import { useRequestStore } from "../stores/requestStore";
import { useSavedResponseStore } from "../stores/savedResponseStore";
import { statusColorClass, formatBytes } from "../types";
import type { Header, SavedResponse } from "../types";

// ---------------------------------------------------------------------------
// SVG icons — stroke-based, strokeWidth 1.5, round caps, consistent with Sidebar
// ---------------------------------------------------------------------------

const IconSave = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M3 2h8l2 2v9a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1Z"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M5 2v4h6V2M5 9h6"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ResponseTab = "pretty" | "raw" | "headers" | "saved";

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function ResponseViewer({ style }: { style?: React.CSSProperties }) {
  const { response, sending, draft } = useRequestStore();
  const loadForRequest = useSavedResponseStore((s) => s.loadForRequest);
  const doSave = useSavedResponseStore((s) => s.saveResponse);
  const byRequest = useSavedResponseStore((s) => s.byRequest);

  const [activeTab, setActiveTab] = useState<ResponseTab>("pretty");
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);

  const requestId = draft.id;
  useEffect(() => {
    if (requestId) {
      loadForRequest(requestId);
    }
    setActiveTab("pretty");
  }, [requestId]); // eslint-disable-line react-hooks/exhaustive-deps

  // When a new response arrives, switch back to "pretty" tab.
  useEffect(() => {
    if (response) setActiveTab("pretty");
  }, [response]);

  const handleSave = async () => {
    if (!response || !requestId) return;
    setSaving(true);
    try {
      await doSave(requestId, response);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 2000);
    } finally {
      setSaving(false);
    }
  };

  const savedResponses = requestId ? (byRequest[requestId] ?? []) : [];

  return (
    <section className="app-response" style={style} aria-label="Response">
      {/* ── 17.1  Status bar ─────────────────────────────────────────────── */}
      <div className="app-response__header">
        <span className="app-response__title">Response</span>

        {response && (
          <>
            <span
              className={`response-status ${statusColorClass(response.status_code)}`}
              aria-label={`HTTP status ${response.status_code}`}
            >
              {response.status_code} {response.status_text}
            </span>

            <div className="response-meta">
              <span
                className="response-meta__item"
                title="Response time"
                aria-label={`Response time ${response.response_time_ms} milliseconds`}
              >
                ⏱ {response.response_time_ms} ms
              </span>
              <span className="response-meta__sep" aria-hidden="true">·</span>
              <span
                className="response-meta__item"
                title="Response size"
                aria-label={`Response size ${formatBytes(response.response_size_bytes)}`}
              >
                {formatBytes(response.response_size_bytes)}
              </span>
            </div>

            {/* Save response button — only shown when a saved request is open */}
            {requestId && (
              <button
                className={`save-response-btn${saveSuccess ? " save-response-btn--saved" : ""}`}
                onClick={handleSave}
                disabled={saving}
                title={requestId ? "Save this response" : "Open a saved request to save responses"}
                aria-label={saveSuccess ? "Response saved!" : "Save response"}
              >
                {saveSuccess ? "✓ Saved" : saving ? "Saving…" : <><IconSave /> Save</>}
              </button>
            )}
          </>
        )}
      </div>

      {/* ── Tab bar (shown only when a response is present OR there are saved responses) ── */}
      {(response || savedResponses.length > 0) && (
        <div className="tab-bar" role="tablist" aria-label="Response tabs">
          {response && (
            <>
              <ResponseTabButton
                id="pretty"
                label="Pretty"
                active={activeTab === "pretty"}
                onClick={() => setActiveTab("pretty")}
              />
              <ResponseTabButton
                id="raw"
                label="Raw"
                active={activeTab === "raw"}
                onClick={() => setActiveTab("raw")}
              />
              <ResponseTabButton
                id="headers"
                label="Headers"
                active={activeTab === "headers"}
                badge={response.headers.length || undefined}
                onClick={() => setActiveTab("headers")}
              />
            </>
          )}
          {requestId && (
            <ResponseTabButton
              id="saved"
              label="Saved"
              active={activeTab === "saved"}
              badge={savedResponses.length || undefined}
              onClick={() => setActiveTab("saved")}
            />
          )}
        </div>
      )}

      {/* ── Content area ─────────────────────────────────────────────────── */}
      <div className="app-response__content">
        {/* Loading state */}
        {sending && (
          <div className="app-response__placeholder" role="status" aria-live="polite">
            <span style={{ fontSize: 24 }} aria-hidden="true">⏳</span>
            <p>Sending request…</p>
          </div>
        )}

        {/* Empty state */}
        {!sending && !response && activeTab !== "saved" && (
          <div className="app-response__placeholder">
            <span style={{ fontSize: 32, opacity: 0.35 }} aria-hidden="true">📭</span>
            <p>Send a request to see the response here.</p>
          </div>
        )}

        {/* Response content — current response tabs */}
        {!sending && response && activeTab !== "saved" && (
          <>
            {activeTab === "pretty" && (
              <div
                role="tabpanel"
                id="tabpanel-pretty"
                aria-labelledby="tab-pretty"
                className="response-body-panel"
              >
                <PrettyBody body={response.body} />
              </div>
            )}

            {activeTab === "raw" && (
              <div
                role="tabpanel"
                id="tabpanel-raw"
                aria-labelledby="tab-raw"
                className="response-body-panel"
              >
                <RawBody body={response.body} />
              </div>
            )}

            {activeTab === "headers" && (
              <div
                role="tabpanel"
                id="tabpanel-headers"
                aria-labelledby="tab-headers"
              >
                <ResponseHeadersTable headers={response.headers} />
              </div>
            )}
          </>
        )}

        {/* Saved responses tab */}
        {activeTab === "saved" && requestId && (
          <div
            role="tabpanel"
            id="tabpanel-saved"
            aria-labelledby="tab-saved"
          >
            <SavedResponsesList requestId={requestId} />
          </div>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Saved responses list
// ---------------------------------------------------------------------------

function SavedResponsesList({ requestId }: { requestId: string }) {
  const { byRequest, rename, remove } = useSavedResponseStore();
  const responses = byRequest[requestId] ?? [];
  const [selected, setSelected] = useState<SavedResponse | null>(null);
  const [selectedTab, setSelectedTab] = useState<"pretty" | "raw" | "headers">("pretty");

  if (responses.length === 0) {
    return (
      <div className="app-response__placeholder">
        <span style={{ fontSize: 32, opacity: 0.35 }} aria-hidden="true">🗂️</span>
        <p>No saved responses yet. Send a request and click <strong>💾 Save</strong>.</p>
      </div>
    );
  }

  return (
    <div className="saved-responses">
      {/* Left: list of saved snapshots */}
      <div className="saved-responses__list" role="listbox" aria-label="Saved responses">
        {responses.map((sr) => (
          <SavedResponseItem
            key={sr.id}
            item={sr}
            isSelected={selected?.id === sr.id}
            onSelect={() => {
              setSelected(sr);
              setSelectedTab("pretty");
            }}
            onRename={(newLabel) => rename(sr.id, requestId, newLabel)}
            onDelete={() => {
              remove(sr.id, requestId);
              if (selected?.id === sr.id) setSelected(null);
            }}
          />
        ))}
      </div>

      {/* Right: response detail */}
      {selected && (
        <div className="saved-responses__detail">
          {/* Mini status bar */}
          <div className="saved-responses__detail-header">
            <span
              className={`response-status ${statusColorClass(selected.status_code)}`}
            >
              {selected.status_code} {selected.status_text}
            </span>
            <div className="response-meta">
              <span className="response-meta__item">⏱ {selected.response_time_ms} ms</span>
              <span className="response-meta__sep" aria-hidden="true">·</span>
              <span className="response-meta__item">{formatBytes(selected.response_size_bytes)}</span>
            </div>
          </div>

          {/* Mini tab bar */}
          <div className="tab-bar tab-bar--sm" role="tablist">
            <ResponseTabButton id="pretty" label="Pretty" active={selectedTab === "pretty"} onClick={() => setSelectedTab("pretty")} />
            <ResponseTabButton id="raw" label="Raw" active={selectedTab === "raw"} onClick={() => setSelectedTab("raw")} />
            <ResponseTabButton id="headers" label="Headers" active={selectedTab === "headers"} badge={selected.headers.length || undefined} onClick={() => setSelectedTab("headers")} />
          </div>

          <div className="saved-responses__detail-body response-body-panel">
            {selectedTab === "pretty" && <PrettyBody body={selected.body} />}
            {selectedTab === "raw" && <RawBody body={selected.body} />}
            {selectedTab === "headers" && <ResponseHeadersTable headers={selected.headers} />}
          </div>
        </div>
      )}

      {/* Prompt when nothing selected */}
      {!selected && (
        <div className="app-response__placeholder saved-responses__pick-hint">
          <span style={{ fontSize: 28, opacity: 0.35 }} aria-hidden="true">👈</span>
          <p>Select a saved response to view it.</p>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Single saved-response list item (with inline rename)
// ---------------------------------------------------------------------------

interface SavedResponseItemProps {
  item: SavedResponse;
  isSelected: boolean;
  onSelect: () => void;
  onRename: (newLabel: string) => void;
  onDelete: () => void;
}

function SavedResponseItem({ item, isSelected, onSelect, onRename, onDelete }: SavedResponseItemProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(item.label);
  const inputRef = useRef<HTMLInputElement>(null);

  const startEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    setDraft(item.label);
    setEditing(true);
    setTimeout(() => inputRef.current?.select(), 0);
  };

  const commitEdit = () => {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== item.label) {
      onRename(trimmed);
    }
    setEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") commitEdit();
    if (e.key === "Escape") { setDraft(item.label); setEditing(false); }
  };

  return (
    <div
      className={`saved-response-item${isSelected ? " saved-response-item--selected" : ""}`}
      role="option"
      aria-selected={isSelected}
      onClick={onSelect}
    >
      <span className={`saved-response-item__status ${statusColorClass(item.status_code)}`}>
        {item.status_code}
      </span>

      {editing ? (
        <input
          ref={inputRef}
          className="saved-response-item__rename-input"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitEdit}
          onKeyDown={handleKeyDown}
          onClick={(e) => e.stopPropagation()}
          aria-label="Rename saved response"
          autoFocus
        />
      ) : (
        <span className="saved-response-item__label">{item.label}</span>
      )}

      <div className="saved-response-item__actions">
        <button
          className="saved-response-item__action-btn"
          title="Rename"
          aria-label={`Rename "${item.label}"`}
          onClick={startEdit}
        >
          ✏️
        </button>
        <button
          className="saved-response-item__action-btn saved-response-item__action-btn--danger"
          title="Delete"
          aria-label={`Delete "${item.label}"`}
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
        >
          🗑️
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared copy-to-clipboard hook
// ---------------------------------------------------------------------------

function useCopyToClipboard() {
  const [copied, setCopied] = useState(false);

  const copy = useCallback((text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, []);

  return { copied, copy };
}

// ---------------------------------------------------------------------------
// Shared copy button component
// ---------------------------------------------------------------------------

function CopyButton({ text }: { text: string }) {
  const { copied, copy } = useCopyToClipboard();

  return (
    <button
      className={`copy-btn${copied ? " copy-btn--copied" : ""}`}
      onClick={() => copy(text)}
      aria-label={copied ? "Copied!" : "Copy to clipboard"}
      title={copied ? "Copied!" : "Copy to clipboard"}
    >
      {copied ? "✓ Copied" : "Copy"}
    </button>
  );
}

// ---------------------------------------------------------------------------
// 17.2 – Pretty body viewer
// ---------------------------------------------------------------------------

function PrettyBody({ body }: { body: string }) {
  const [epoch, setEpoch] = useState(0);
  const [allOpen, setAllOpen] = useState<boolean | null>(null);

  const triggerAll = (open: boolean) => {
    setAllOpen(open);
    setEpoch(e => e + 1);
  };

  let content: React.ReactNode;
  let copyText = body;
  let isJson = false;

  try {
    const parsed = JSON.parse(body);
    const pretty = JSON.stringify(parsed, null, 2);
    copyText = pretty;
    isJson = true;
    content = (
      <JsonTreeContext.Provider value={{ epoch, allOpen }}>
        <pre aria-label="Response body (JSON)" className="response-body-pre json-tree">
          <JsonNode value={parsed} depth={0} isLast={true} />
        </pre>
      </JsonTreeContext.Provider>
    );
  } catch {
    content = (
      <>
        <p className="response-body-hint">
          Body is not valid JSON. Displaying as plain text.
        </p>
        <pre aria-label="Response body (raw)" className="response-body-pre">
          {body}
        </pre>
      </>
    );
  }

  return (
    <div className="response-body-pretty" aria-label="Response body (pretty)">
      <div className="response-body-toolbar response-body-toolbar--sticky">
        {isJson && (
          <>
            <button
              className="json-tree-btn"
              onClick={() => triggerAll(true)}
              title="Expand all nodes"
              aria-label="Expand all"
            >
              ＋ Expand all
            </button>
            <button
              className="json-tree-btn"
              onClick={() => triggerAll(false)}
              title="Collapse all nodes"
              aria-label="Collapse all"
            >
              － Collapse all
            </button>
            <span className="response-body-toolbar__sep" aria-hidden="true" />
          </>
        )}
        <CopyButton text={copyText} />
      </div>
      {content}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 17.2 – Raw body viewer
// ---------------------------------------------------------------------------

function RawBody({ body }: { body: string }) {
  return (
    <div className="response-body-raw">
      <div className="response-body-toolbar response-body-toolbar--sticky">
        <CopyButton text={body} />
      </div>
      <pre aria-label="Response body (raw)" className="response-body-pre">
        {body || <span className="response-body-empty">(empty body)</span>}
      </pre>
    </div>
  );
}

// ---------------------------------------------------------------------------
// 17.2 – Interactive JSON tree (expand / collapse)
// ---------------------------------------------------------------------------

type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

interface JsonTreeCtx {
  epoch: number;
  allOpen: boolean | null;
}

const JsonTreeContext = createContext<JsonTreeCtx>({ epoch: 0, allOpen: null });

interface JsonNodeProps {
  value: JsonValue;
  depth: number;
  isLast: boolean;
  propKey?: string;
}

function JsonNode({ value, depth, isLast, propKey }: JsonNodeProps) {
  const { epoch, allOpen } = useContext(JsonTreeContext);
  const [open, setOpen] = useState(depth < 2);

  useEffect(() => {
    if (allOpen !== null) setOpen(allOpen);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [epoch]);

  const toggle = () => setOpen(o => !o);
  const indent = "  ".repeat(depth);
  const trailing = isLast ? "" : ",";

  const keyEl = propKey !== undefined ? (
    <><span className="json-key">{JSON.stringify(propKey)}</span><span className="json-punctuation">: </span></>
  ) : null;

  if (value === null) {
    return <span className="json-line">{indent}{keyEl}<span className="json-null">null</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
  }
  if (typeof value === "boolean") {
    return <span className="json-line">{indent}{keyEl}<span className="json-boolean">{String(value)}</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
  }
  if (typeof value === "number") {
    return <span className="json-line">{indent}{keyEl}<span className="json-number">{value}</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
  }
  if (typeof value === "string") {
    return <span className="json-line">{indent}{keyEl}<span className="json-string">{JSON.stringify(value)}</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
  }

  if (Array.isArray(value)) {
    if (value.length === 0) {
      return <span className="json-line">{indent}{keyEl}<span className="json-punctuation">[]</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
    }
    const preview = `[ ${value.length} ${value.length === 1 ? "item" : "items"} ]`;
    return (
      <>
        <span className="json-line">
          {indent}{keyEl}
          <button className="json-toggle" onClick={toggle} aria-label={open ? "Collapse array" : "Expand array"} aria-expanded={open}>
            {open ? "▾" : "▸"}
          </button>
          <span className="json-punctuation">[</span>
          {!open && <><span className="json-collapsed-hint"> {preview} </span><span className="json-punctuation">]{trailing}</span></>}
          {"\n"}
        </span>
        {open && (
          <>
            {value.map((item, i) => (
              <JsonNode key={i} value={item as JsonValue} depth={depth + 1} isLast={i === value.length - 1} />
            ))}
            <span className="json-line">{indent}<span className="json-punctuation">]{trailing}</span>{"\n"}</span>
          </>
        )}
      </>
    );
  }

  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, JsonValue>);
    if (entries.length === 0) {
      return <span className="json-line">{indent}{keyEl}<span className="json-punctuation">{"{}"}</span><span className="json-punctuation">{trailing}</span>{"\n"}</span>;
    }
    const preview = `{ ${entries.length} ${entries.length === 1 ? "key" : "keys"} }`;
    return (
      <>
        <span className="json-line">
          {indent}{keyEl}
          <button className="json-toggle" onClick={toggle} aria-label={open ? "Collapse object" : "Expand object"} aria-expanded={open}>
            {open ? "▾" : "▸"}
          </button>
          <span className="json-punctuation">{"{"}</span>
          {!open && <><span className="json-collapsed-hint"> {preview} </span><span className="json-punctuation">{"}"}{trailing}</span></>}
          {"\n"}
        </span>
        {open && (
          <>
            {entries.map(([k, v], i) => (
              <JsonNode key={k} value={v} depth={depth + 1} isLast={i === entries.length - 1} propKey={k} />
            ))}
            <span className="json-line">{indent}<span className="json-punctuation">{"}"}{trailing}</span>{"\n"}</span>
          </>
        )}
      </>
    );
  }

  return <span>{indent}{String(value)}{trailing}{"\n"}</span>;
}

// ---------------------------------------------------------------------------
// 17.3 – Response headers table
// ---------------------------------------------------------------------------

function ResponseHeadersTable({ headers }: { headers: Header[] }) {
  if (headers.length === 0) {
    return <p className="response-headers-empty">No response headers.</p>;
  }

  return (
    <table className="response-headers-table" aria-label="Response headers">
      <thead>
        <tr>
          <th scope="col" className="response-headers-table__th">Header</th>
          <th scope="col" className="response-headers-table__th">Value</th>
        </tr>
      </thead>
      <tbody>
        {headers.map((h, i) => (
          <tr key={i} className="response-headers-table__row">
            <td className="response-headers-table__key">{h.key}</td>
            <td className="response-headers-table__value">{h.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

interface ResponseTabButtonProps {
  id: string;
  label: string;
  active: boolean;
  badge?: number;
  onClick: () => void;
}

function ResponseTabButton({ id, label, active, badge, onClick }: ResponseTabButtonProps) {
  return (
    <button
      role="tab"
      id={`tab-${id}`}
      aria-selected={active}
      aria-controls={`tabpanel-${id}`}
      className={`tab-bar__tab${active ? " tab-bar__tab--active" : ""}`}
      onClick={onClick}
    >
      {label}
      {badge !== undefined && badge > 0 && (
        <span className="tab-bar__badge" aria-label={`${badge}`}>
          {badge}
        </span>
      )}
    </button>
  );
}
