// Request builder panel — full implementation (task 16).
// - 16.1: URL bar with method selector and {{var}} variable highlighting
// - 16.2: Tab panels: Headers, Params, Body, Auth
//
// Requirements: 1.1, 1.3, 3.4, 9.1

import { useState, useRef, useCallback, useMemo, useEffect } from "react";
import { useRequestStore, extractPathParams, parseQueryString, replaceQueryString } from "../stores/requestStore";
import { useEnvStore } from "../stores/envStore";
import { useCollectionStore } from "../stores/collectionStore";
import {
  type Header,
  type QueryParam,
  type RequestBody,
  type AuthConfig,
  type MultipartField,
  type ApiKeyLocation,
} from "../types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = "headers" | "params" | "body" | "auth";

// ---------------------------------------------------------------------------
// useValidVariableSet — derives the set of valid variable keys from envStore.
// A key is "valid" if it exists in globalVariables OR in the active
// environment's variables, and is enabled.
// ---------------------------------------------------------------------------

function useValidVariableSet(): Set<string> {
  const { activeEnvironmentId, variablesByEnv, globalVariables } = useEnvStore();

  return useMemo(() => {
    const keys = new Set<string>();
    for (const v of globalVariables) {
      if (v.enabled) keys.add(v.key);
    }
    if (activeEnvironmentId) {
      const envVars = variablesByEnv[activeEnvironmentId] ?? [];
      for (const v of envVars) {
        if (v.enabled) keys.add(v.key);
      }
    }
    return keys;
  }, [activeEnvironmentId, variablesByEnv, globalVariables]);
}

// ---------------------------------------------------------------------------
// buildJsonHighlightedHtml — tokenises JSON and wraps each token in a
// <span> with a colour class, then layers {{var}} highlights on top.
// Falls back gracefully when the text is not valid JSON (e.g. mid-edit).
// ---------------------------------------------------------------------------

function buildJsonHighlightedHtml(text: string, validKeys: Set<string>): string {
  // Tokenise manually so we have full control and add zero dependencies.
  // We walk char-by-char and emit coloured spans.
  interface Token {
    type:
      | "key"        // object key string
      | "string"     // value string
      | "number"
      | "boolean"
      | "null"
      | "punctuation" // { } [ ] : ,
      | "whitespace"
      | "unknown";    // parse error / raw char
    raw: string;      // the literal text (may contain {{var}})
  }

  const tokens: Token[] = [];
  let i = 0;
  const len = text.length;

  // We track whether the next string literal is an object key or a value.
  // A string after '{' start or ',' inside an object (and before ':') is a key.
  // Simple heuristic: scan backwards through whitespace — if prior non-ws token
  // was '{' or ',', this string is a key candidate; confirm by checking the
  // following non-ws token is ':'.
  // Because we build tokens linearly we use a lookahead instead.

  function isNextNonWsColon(startAfter: number): boolean {
    let j = startAfter;
    while (j < len && (text[j] === " " || text[j] === "\t" || text[j] === "\n" || text[j] === "\r")) j++;
    return text[j] === ":";
  }

  while (i < len) {
    const ch = text[i];

    // Whitespace
    if (ch === " " || ch === "\t" || ch === "\n" || ch === "\r") {
      let ws = "";
      while (i < len && (text[i] === " " || text[i] === "\t" || text[i] === "\n" || text[i] === "\r")) {
        ws += text[i++];
      }
      tokens.push({ type: "whitespace", raw: ws });
      continue;
    }

    // String (or key)
    if (ch === '"') {
      let str = '"';
      i++;
      while (i < len) {
        if (text[i] === "\\") {
          str += text[i] + (text[i + 1] ?? "");
          i += 2;
        } else if (text[i] === '"') {
          str += '"';
          i++;
          break;
        } else {
          str += text[i++];
        }
      }
      // Determine key vs string value
      const type: Token["type"] = isNextNonWsColon(i) ? "key" : "string";
      tokens.push({ type, raw: str });
      continue;
    }

    // Punctuation
    if (ch === "{" || ch === "}" || ch === "[" || ch === "]" || ch === ":" || ch === ",") {
      tokens.push({ type: "punctuation", raw: ch });
      i++;
      continue;
    }

    // Number
    if ((ch >= "0" && ch <= "9") || ch === "-") {
      let num = "";
      while (i < len && !/[\s,\}\]\{]/.test(text[i])) {
        num += text[i++];
      }
      tokens.push({ type: "number", raw: num });
      continue;
    }

    // true / false
    if (text.startsWith("true", i) && !/[a-zA-Z0-9_]/.test(text[i + 4] ?? "")) {
      tokens.push({ type: "boolean", raw: "true" });
      i += 4;
      continue;
    }
    if (text.startsWith("false", i) && !/[a-zA-Z0-9_]/.test(text[i + 5] ?? "")) {
      tokens.push({ type: "boolean", raw: "false" });
      i += 5;
      continue;
    }

    // null
    if (text.startsWith("null", i) && !/[a-zA-Z0-9_]/.test(text[i + 4] ?? "")) {
      tokens.push({ type: "null", raw: "null" });
      i += 4;
      continue;
    }

    // Unknown / raw character (parser hasn't matched anything — handle gracefully)
    tokens.push({ type: "unknown", raw: ch });
    i++;
  }

  // Map token types to CSS classes
  const classMap: Record<Token["type"], string> = {
    key:         "json-key",
    string:      "json-string",
    number:      "json-number",
    boolean:     "json-boolean",
    null:        "json-null",
    punctuation: "json-punctuation",
    whitespace:  "",
    unknown:     "",
  };

  // Build HTML: escape, wrap in span, then layer {{var}} highlights inside
  const parts: string[] = [];
  for (const tok of tokens) {
    const cls = classMap[tok.type];
    // Escape HTML in the raw token text
    const escaped = tok.raw
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");

    // Apply {{var}} highlighting inside the token
    const withVars = escaped.replace(/\{\{([^}]*)\}\}/g, (_m, key: string) => {
      const vcls = validKeys.has(key) ? "var-valid" : "var-invalid";
      return `<mark class="var-highlight ${vcls}">{{${key}}}</mark>`;
    });

    if (cls) {
      parts.push(`<span class="${cls}">${withVars}</span>`);
    } else {
      parts.push(withVars);
    }
  }

  return parts.join("");
}

// ---------------------------------------------------------------------------
// buildHighlightedHtml — shared helper.
// Replaces {{key}} with a <mark> that carries a valid/invalid class.
// ---------------------------------------------------------------------------

function buildHighlightedHtml(
  text: string,
  validKeys: Set<string>,
  highlightPathParams = false,
): string {
  // First escape HTML, then apply highlights token by token so the two
  // highlight passes don't interfere with each other.
  const escaped = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // Combined regex: match either :param or {{key}}
  // We split the URL on path/query boundary so we only highlight :param in
  // the path portion (before '?'), not inside query-string values.
  if (highlightPathParams) {
    // Split at '?' to avoid matching `:param` inside query values
    const qIdx = escaped.indexOf("?");
    const pathPart = qIdx === -1 ? escaped : escaped.slice(0, qIdx);
    const queryPart = qIdx === -1 ? "" : escaped.slice(qIdx);

    const highlightedPath = pathPart.replace(
      /:([A-Za-z_][A-Za-z0-9_]*)/g,
      (_m, name: string) =>
        `<mark class="var-highlight path-param-highlight">:${name}</mark>`,
    );

    const highlightedQuery = queryPart.replace(
      /\{\{([^}]*)\}\}/g,
      (_match, key: string) => {
        const cls = validKeys.has(key) ? "var-valid" : "var-invalid";
        return `<mark class="var-highlight ${cls}">{{${key}}}</mark>`;
      },
    );

    // Also highlight {{var}} in path portion
    const highlightedPathWithVars = highlightedPath.replace(
      /\{\{([^}]*)\}\}/g,
      (_match, key: string) => {
        const cls = validKeys.has(key) ? "var-valid" : "var-invalid";
        return `<mark class="var-highlight ${cls}">{{${key}}}</mark>`;
      },
    );

    return highlightedPathWithVars + highlightedQuery;
  }

  return escaped.replace(/\{\{([^}]*)\}\}/g, (_match, key: string) => {
    const cls = validKeys.has(key) ? "var-valid" : "var-invalid";
    return `<mark class="var-highlight ${cls}">{{${key}}}</mark>`;
  });
}

// ---------------------------------------------------------------------------
// Variable-highlighting URL display
// Renders an overlay with {{var}} spans highlighted while keeping a real
// transparent <input> on top for editing.
// ---------------------------------------------------------------------------

function VariableHighlightInput({
  value,
  onChange,
  placeholder,
  validKeys,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  validKeys: Set<string>;
}) {
  const containerRef = useRef<HTMLDivElement>(null);

  const highlighted = buildHighlightedHtml(value, validKeys, true);

  return (
    <div className="var-input-wrapper" ref={containerRef}>
      {/* Highlight layer (aria-hidden, not interactive) */}
      <div
        className="var-input-highlight"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: highlighted || "" }}
      />
      {/* Transparent real input on top */}
      <input
        className="var-input-real input"
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label="Request URL"
        spellCheck={false}
        autoComplete="off"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// VarHighlightInlineInput — same overlay trick but for inline cells
// (headers, query params, form fields).
// ---------------------------------------------------------------------------

function VarHighlightInlineInput({
  value,
  onChange,
  placeholder,
  ariaLabel,
  validKeys,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  validKeys: Set<string>;
}) {
  const highlighted = buildHighlightedHtml(value, validKeys);

  return (
    <div className="var-input-wrapper var-input-wrapper--inline">
      <div
        className="var-input-highlight"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: highlighted || "" }}
      />
      <input
        className="var-input-real input kv-table__input"
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={ariaLabel}
        spellCheck={false}
        autoComplete="off"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// VarHighlightTextarea — overlay trick for <textarea> (JSON / raw body).
// ---------------------------------------------------------------------------

function VarHighlightTextarea({
  value,
  onChange,
  placeholder,
  ariaLabel,
  validKeys,
  className,
  jsonMode = false,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  validKeys: Set<string>;
  className?: string;
  jsonMode?: boolean;
}) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLDivElement>(null);

  // Sync scroll between highlight div and textarea
  const syncScroll = () => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  };

  const highlighted = jsonMode
    ? buildJsonHighlightedHtml(value, validKeys)
    : buildHighlightedHtml(value, validKeys);

  return (
    <div className="var-textarea-wrapper">
      <div
        ref={highlightRef}
        className="var-textarea-highlight"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: highlighted + "\n" /* trailing newline prevents layout jump */ }}
      />
      <textarea
        ref={textareaRef}
        className={`var-textarea-real ${className ?? ""}`}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onScroll={syncScroll}
        placeholder={placeholder}
        aria-label={ariaLabel}
        spellCheck={false}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// KeyValueTable — reusable table for headers and query params
// ---------------------------------------------------------------------------

interface KeyValueRow {
  key: string;
  value: string;
  enabled: boolean;
}

function KeyValueTable<T extends KeyValueRow>({
  rows,
  onChange,
  keyPlaceholder,
  valuePlaceholder,
  ariaLabel,
  validKeys,
}: {
  rows: T[];
  onChange: (rows: T[]) => void;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  ariaLabel: string;
  validKeys?: Set<string>;
}) {
  const update = (index: number, patch: Partial<T>) => {
    const next = rows.map((r, i) => (i === index ? { ...r, ...patch } : r));
    onChange(next as T[]);
  };

  const remove = (index: number) => {
    onChange(rows.filter((_, i) => i !== index) as T[]);
  };

  const add = () => {
    onChange([...rows, { key: "", value: "", enabled: true } as T]);
  };

  return (
    <div className="kv-table" role="table" aria-label={ariaLabel}>
      {/* Header row */}
      <div className="kv-table__header" role="row">
        <span role="columnheader" className="kv-table__cell-enable" />
        <span role="columnheader" className="kv-table__cell-key">
          Key
        </span>
        <span role="columnheader" className="kv-table__cell-value">
          Value
        </span>
        <span role="columnheader" className="kv-table__cell-action" />
      </div>

      {/* Data rows */}
      {rows.map((row, i) => (
        <div
          key={i}
          className={`kv-table__row${row.enabled ? "" : " kv-table__row--disabled"}`}
          role="row"
        >
          <span className="kv-table__cell-enable" role="cell">
            <input
              type="checkbox"
              checked={row.enabled}
              onChange={(e) => update(i, { enabled: e.target.checked } as Partial<T>)}
              aria-label={`Enable row ${i + 1}`}
            />
          </span>
          <span className="kv-table__cell-key" role="cell">
            <input
              className="input kv-table__input"
              type="text"
              value={row.key}
              onChange={(e) => update(i, { key: e.target.value } as Partial<T>)}
              placeholder={keyPlaceholder ?? "Key"}
              aria-label={`Row ${i + 1} key`}
            />
          </span>
          <span className="kv-table__cell-value" role="cell">
            {validKeys ? (
              <VarHighlightInlineInput
                value={row.value}
                onChange={(v) => update(i, { value: v } as Partial<T>)}
                placeholder={valuePlaceholder ?? "Value"}
                ariaLabel={`Row ${i + 1} value`}
                validKeys={validKeys}
              />
            ) : (
              <input
                className="input kv-table__input"
                type="text"
                value={row.value}
                onChange={(e) => update(i, { value: e.target.value } as Partial<T>)}
                placeholder={valuePlaceholder ?? "Value"}
                aria-label={`Row ${i + 1} value`}
              />
            )}
          </span>
          <span className="kv-table__cell-action" role="cell">
            <button
              className="btn btn--ghost btn--icon"
              onClick={() => remove(i)}
              aria-label={`Remove row ${i + 1}`}
              title="Remove"
            >
              ✕
            </button>
          </span>
        </div>
      ))}

      {/* Add row button */}
      <div className="kv-table__add">
        <button className="btn btn--ghost" onClick={add}>
          + Add Row
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// MultipartTable — same as KV but for MultipartField arrays
// ---------------------------------------------------------------------------

function MultipartTable({
  rows,
  onChange,
  validKeys,
}: {
  rows: MultipartField[];
  onChange: (rows: MultipartField[]) => void;
  validKeys?: Set<string>;
}) {
  return (
    <KeyValueTable<MultipartField>
      rows={rows}
      onChange={onChange}
      keyPlaceholder="Field name"
      valuePlaceholder="Value"
      ariaLabel="Multipart fields"
      validKeys={validKeys}
    />
  );
}

// ---------------------------------------------------------------------------
// ContentTypeInput — combobox with grouped quick-pick suggestions
// ---------------------------------------------------------------------------

const CONTENT_TYPE_GROUPS = [
  {
    label: "Application",
    options: [
      "application/json",
      "application/xml",
      "application/x-www-form-urlencoded",
      "application/octet-stream",
      "application/graphql",
      "application/ld+json",
      "application/x-ndjson",
      "application/yaml",
      "application/toml",
    ],
  },
  {
    label: "Text",
    options: [
      "text/plain",
      "text/html",
      "text/xml",
      "text/csv",
      "text/markdown",
    ],
  },
  {
    label: "Multipart",
    options: ["multipart/form-data", "multipart/mixed"],
  },
] as const;

function ContentTypeInput({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const pick = (ct: string) => {
    onChange(ct);
    setOpen(false);
    inputRef.current?.focus();
  };

  // Filter suggestions based on current input value
  const query = value.toLowerCase();
  const filtered = CONTENT_TYPE_GROUPS
    .map((g) => ({
      label: g.label,
      options: g.options.filter((o) => o.includes(query)),
    }))
    .filter((g) => g.options.length > 0);

  const hasResults = filtered.length > 0;

  return (
    <div className="ct-input" ref={wrapperRef}>
      <div className="ct-input__field">
        <input
          ref={inputRef}
          id="raw-content-type"
          className="input ct-input__text"
          type="text"
          value={value}
          onChange={(e) => { onChange(e.target.value); setOpen(true); }}
          onFocus={() => setOpen(true)}
          placeholder="text/plain"
          autoComplete="off"
          spellCheck={false}
          aria-label="Content-Type"
          aria-expanded={open}
          aria-haspopup="listbox"
          role="combobox"
        />
        <button
          type="button"
          className="ct-input__toggle"
          aria-label="Show content-type suggestions"
          tabIndex={-1}
          onClick={() => { setOpen((v) => !v); inputRef.current?.focus(); }}
        >
          <svg width="10" height="6" viewBox="0 0 10 6" aria-hidden="true">
            <path d="M1 1l4 4 4-4" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </button>
      </div>

      {open && (
        <div className="ct-input__dropdown" role="listbox" aria-label="Content-type suggestions">
          {hasResults ? (
            filtered.map((group) => (
              <div key={group.label} className="ct-input__group">
                <div className="ct-input__group-label">{group.label}</div>
                {group.options.map((opt) => (
                  <button
                    key={opt}
                    type="button"
                    role="option"
                    aria-selected={value === opt}
                    className={`ct-input__option${value === opt ? " ct-input__option--selected" : ""}`}
                    onMouseDown={(e) => { e.preventDefault(); pick(opt); }}
                  >
                    {opt}
                  </button>
                ))}
              </div>
            ))
          ) : (
            <div className="ct-input__empty">No suggestions</div>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// BodyEditor
// ---------------------------------------------------------------------------

function BodyEditor({
  body,
  onChange,
  validKeys,
}: {
  body: RequestBody;
  onChange: (body: RequestBody) => void;
  validKeys: Set<string>;
}) {
  const [jsonFormatError, setJsonFormatError] = useState<string | null>(null);

  // Drafts — preserve content when switching between body types so users
  // don't lose their work. Text content is shared between json/raw; fields
  // are shared between form_url_encoded/multipart.
  const drafts = useRef<{
    textContent: string;
    rawContentType: string;
    fields: import("../types").MultipartField[];
    binaryPath: string;
  }>({
    textContent: body.type === "json" || body.type === "raw" ? (body as { content: string }).content : "",
    rawContentType: body.type === "raw" ? (body as { content_type: string }).content_type : "text/plain",
    fields: body.type === "form_url_encoded" || body.type === "multipart" ? (body as { fields: import("../types").MultipartField[] }).fields : [],
    binaryPath: body.type === "binary" ? (body as { file_path: string }).file_path : "",
  });

  // Keep drafts in sync with the current body as the user edits.
  if (body.type === "json" || body.type === "raw") {
    drafts.current.textContent = (body as { content: string }).content;
  }
  if (body.type === "raw") {
    drafts.current.rawContentType = (body as { content_type: string }).content_type;
  }
  if (body.type === "form_url_encoded" || body.type === "multipart") {
    drafts.current.fields = (body as { fields: import("../types").MultipartField[] }).fields;
  }
  if (body.type === "binary") {
    drafts.current.binaryPath = (body as { file_path: string }).file_path;
  }

  const formatJson = () => {
    if (body.type !== "json") return;
    try {
      const parsed = JSON.parse(body.content);
      const pretty = JSON.stringify(parsed, null, 2);
      onChange({ type: "json", content: pretty });
      setJsonFormatError(null);
    } catch (e) {
      setJsonFormatError(e instanceof Error ? e.message : "Invalid JSON");
    }
  };

  const bodyTypes = [
    { id: "none", label: "None" },
    { id: "json", label: "JSON" },
    { id: "form_url_encoded", label: "Form (URL encoded)" },
    { id: "multipart", label: "Form (multipart)" },
    { id: "raw", label: "Raw" },
    { id: "binary", label: "Binary" },
  ] as const;

  type BodyTypeId = (typeof bodyTypes)[number]["id"];

  const switchType = (newType: BodyTypeId) => {
    if (newType === body.type) return;
    switch (newType) {
      case "none":
        onChange({ type: "none" });
        break;
      case "json":
        // Restore last text content
        onChange({ type: "json", content: drafts.current.textContent });
        break;
      case "form_url_encoded":
        // Restore last fields
        onChange({ type: "form_url_encoded", fields: drafts.current.fields });
        break;
      case "multipart":
        // Restore last fields
        onChange({ type: "multipart", fields: drafts.current.fields });
        break;
      case "raw":
        // Restore last text content and content-type
        onChange({ type: "raw", content: drafts.current.textContent, content_type: drafts.current.rawContentType });
        break;
      case "binary":
        onChange({ type: "binary", file_path: drafts.current.binaryPath });
        break;
    }
  };

  return (
    <div className="body-editor">
      {/* Body type selector */}
      <div className="body-editor__type-bar">
        {bodyTypes.map(({ id, label }) => (
          <button
            key={id}
            className={`btn btn--ghost body-editor__type-btn${body.type === id ? " body-editor__type-btn--active" : ""}`}
            onClick={() => switchType(id)}
            aria-pressed={body.type === id}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Body content area */}
      <div className="body-editor__content">
        {body.type === "none" && (
          <p className="body-editor__empty-note">
            This request has no body.
          </p>
        )}

        {body.type === "json" && (
          <div className="body-editor__json-wrapper">
            <div className="body-editor__json-toolbar">
              <button
                className="btn btn--ghost btn--sm"
                onClick={formatJson}
                title="Beautify / format JSON"
                aria-label="Format JSON"
                type="button"
              >
                ✦ Format
              </button>
              {jsonFormatError && (
                <span className="body-editor__json-error" role="alert">
                  {jsonFormatError}
                </span>
              )}
            </div>
            <VarHighlightTextarea
              value={body.content}
              onChange={(content) => {
                onChange({ type: "json", content });
                if (jsonFormatError) setJsonFormatError(null);
              }}
              placeholder='{"key": "value"}'
              ariaLabel="JSON body"
              validKeys={validKeys}
              className="body-editor__textarea"
              jsonMode={true}
            />
          </div>
        )}

        {body.type === "raw" && (
          <div className="body-editor__raw-wrapper">
            <div className="body-editor__raw-ct">
              <label htmlFor="raw-content-type" className="form-label">
                Content-Type
              </label>
              <ContentTypeInput
                value={body.content_type}
                onChange={(ct) => onChange({ type: "raw", content: body.content, content_type: ct })}
              />
            </div>
            <VarHighlightTextarea
              value={body.content}
              onChange={(content) =>
                onChange({ type: "raw", content, content_type: body.content_type })
              }
              placeholder="Raw body content…"
              ariaLabel="Raw body"
              validKeys={validKeys}
              className="body-editor__textarea"
            />
          </div>
        )}

        {body.type === "form_url_encoded" && (
          <MultipartTable
            rows={body.fields}
            onChange={(fields) => onChange({ type: "form_url_encoded", fields })}
            validKeys={validKeys}
          />
        )}

        {body.type === "multipart" && (
          <MultipartTable
            rows={body.fields}
            onChange={(fields) => onChange({ type: "multipart", fields })}
            validKeys={validKeys}
          />
        )}

        {body.type === "binary" && (
          <div className="body-editor__binary">
            <label htmlFor="binary-path" className="form-label">
              File path
            </label>
            <input
              id="binary-path"
              className="input"
              type="text"
              value={body.file_path}
              onChange={(e) => onChange({ type: "binary", file_path: e.target.value })}
              placeholder="/path/to/file or C:\path\to\file"
              aria-label="Binary file path"
            />
            <p className="body-editor__binary-hint">
              Enter the absolute path to the file to send as the request body.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// AuthEditor
// ---------------------------------------------------------------------------

function AuthEditor({
  auth,
  onChange,
  showInherit = false,
}: {
  auth: AuthConfig;
  onChange: (auth: AuthConfig) => void;
  /** When true, show the "Inherit" option (for requests/folders; hide for top-level collections). */
  showInherit?: boolean;
}) {
  const authTypes = [
    ...(showInherit ? [{ id: "inherit" as const, label: "Inherit" }] : []),
    { id: "none" as const, label: "No Auth" },
    { id: "basic_auth" as const, label: "Basic Auth" },
    { id: "bearer_token" as const, label: "Bearer Token" },
    { id: "api_key" as const, label: "API Key" },
  ];

  type AuthTypeId = AuthConfig["type"];

  const switchType = (newType: AuthTypeId) => {
    if (newType === auth.type) return;
    switch (newType) {
      case "inherit":
        onChange({ type: "inherit" });
        break;
      case "none":
        onChange({ type: "none" });
        break;
      case "basic_auth":
        onChange({ type: "basic_auth", username: "", password: "" });
        break;
      case "bearer_token":
        onChange({ type: "bearer_token", token: "" });
        break;
      case "api_key":
        onChange({ type: "api_key", key: "", value: "", location: "header" });
        break;
    }
  };

  return (
    <div className="auth-editor">
      {/* Auth type selector */}
      <div className="auth-editor__type-bar">
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

      {/* Auth fields */}
      <div className="auth-editor__fields">
        {auth.type === "inherit" && (
          <p className="auth-editor__empty-note">
            Auth will be inherited from the parent collection or folder. If none is set, no authentication is sent.
          </p>
        )}

        {auth.type === "none" && (
          <p className="auth-editor__empty-note">
            No authorization will be sent with this request.
          </p>
        )}

        {auth.type === "basic_auth" && (
          <div className="auth-editor__grid">
            <div className="form-group">
              <label htmlFor="auth-username" className="form-label">
                Username
              </label>
              <input
                id="auth-username"
                className="input"
                type="text"
                value={auth.username}
                onChange={(e) =>
                  onChange({ type: "basic_auth", username: e.target.value, password: auth.password })
                }
                placeholder="username"
                autoComplete="off"
              />
            </div>
            <div className="form-group">
              <label htmlFor="auth-password" className="form-label">
                Password
              </label>
              <input
                id="auth-password"
                className="input"
                type="password"
                value={auth.password}
                onChange={(e) =>
                  onChange({ type: "basic_auth", username: auth.username, password: e.target.value })
                }
                placeholder="password"
                autoComplete="new-password"
              />
            </div>
            <p className="auth-editor__note">
              Credentials will be Base64-encoded and sent as an{" "}
              <code>Authorization: Basic …</code> header.
            </p>
          </div>
        )}

        {auth.type === "bearer_token" && (
          <div className="form-group" style={{ maxWidth: 480 }}>
            <label htmlFor="auth-token" className="form-label">
              Token
            </label>
            <input
              id="auth-token"
              className="input"
              type="text"
              value={auth.token}
              onChange={(e) => onChange({ type: "bearer_token", token: e.target.value })}
              placeholder="your-token-here"
              autoComplete="off"
            />
            <p className="auth-editor__note" style={{ marginTop: 6 }}>
              Sent as <code>Authorization: Bearer &lt;token&gt;</code>.
            </p>
          </div>
        )}

        {auth.type === "api_key" && (
          <div className="auth-editor__grid">
            <div className="form-group">
              <label htmlFor="api-key-name" className="form-label">
                Key name
              </label>
              <input
                id="api-key-name"
                className="input"
                type="text"
                value={auth.key}
                onChange={(e) =>
                  onChange({ type: "api_key", key: e.target.value, value: auth.value, location: auth.location })
                }
                placeholder="X-API-Key"
                autoComplete="off"
              />
            </div>
            <div className="form-group">
              <label htmlFor="api-key-value" className="form-label">
                Value
              </label>
              <input
                id="api-key-value"
                className="input"
                type="text"
                value={auth.value}
                onChange={(e) =>
                  onChange({ type: "api_key", key: auth.key, value: e.target.value, location: auth.location })
                }
                placeholder="your-api-key"
                autoComplete="off"
              />
            </div>
            <div className="form-group">
              <label className="form-label">Add to</label>
              <div className="auth-editor__location-btns">
                {(["header", "query_param"] as ApiKeyLocation[]).map((loc) => (
                  <button
                    key={loc}
                    className={`btn btn--ghost${auth.location === loc ? " auth-editor__type-btn--active" : ""}`}
                    onClick={() =>
                      onChange({ type: "api_key", key: auth.key, value: auth.value, location: loc })
                    }
                    aria-pressed={auth.location === loc}
                  >
                    {loc === "header" ? "Header" : "Query param"}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// TabBar
// ---------------------------------------------------------------------------

function TabBar({
  active,
  onSelect,
  headerCount,
  paramCount,
}: {
  active: TabId;
  onSelect: (tab: TabId) => void;
  headerCount: number;
  paramCount: number;
}) {
  const tabs: { id: TabId; label: string; badge?: number }[] = [
    { id: "params", label: "Params", badge: paramCount || undefined },
    { id: "headers", label: "Headers", badge: headerCount || undefined },
    { id: "body", label: "Body" },
    { id: "auth", label: "Auth" },
  ];

  return (
    <div className="tab-bar" role="tablist" aria-label="Request builder tabs">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={active === tab.id}
          className={`tab-bar__tab${active === tab.id ? " tab-bar__tab--active" : ""}`}
          onClick={() => onSelect(tab.id)}
        >
          {tab.label}
          {tab.badge !== undefined && (
            <span className="tab-bar__badge" aria-label={`${tab.badge} items`}>
              {tab.badge}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// PathParamsTable — read-only keys, editable values, no add/remove
// ---------------------------------------------------------------------------

function PathParamsTable({
  pathParams,
  onChange,
}: {
  pathParams: { name: string; value: string }[];
  onChange: (pathParams: { name: string; value: string }[]) => void;
}) {
  if (pathParams.length === 0) return null;

  const update = (index: number, value: string) => {
    const next = pathParams.map((p, i) => (i === index ? { ...p, value } : p));
    onChange(next);
  };

  return (
    <div className="path-params-section">
      <div className="path-params-section__header">
        <span className="path-params-section__title">Path Variables</span>
        <span className="path-params-section__hint">Detected from URL (e.g. <code>:id</code>)</span>
      </div>
      <div className="kv-table" role="table" aria-label="Path variables">
        <div className="kv-table__header" role="row">
          <span role="columnheader" className="kv-table__cell-enable" />
          <span role="columnheader" className="kv-table__cell-key">Variable</span>
          <span role="columnheader" className="kv-table__cell-value">Value</span>
          <span role="columnheader" className="kv-table__cell-action" />
        </div>
        {pathParams.map((p, i) => (
          <div key={p.name} className="kv-table__row" role="row">
            <span className="kv-table__cell-enable" role="cell">
              {/* path params are always active */}
              <span className="path-param-dot" aria-hidden="true" />
            </span>
            <span className="kv-table__cell-key" role="cell">
              <span className="path-param-key">:{p.name}</span>
            </span>
            <span className="kv-table__cell-value" role="cell">
              <input
                className="input kv-table__input"
                type="text"
                value={p.value}
                onChange={(e) => update(i, e.target.value)}
                placeholder={`Enter value for :${p.name}`}
                aria-label={`:${p.name} value`}
              />
            </span>
            <span className="kv-table__cell-action" role="cell" />
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RequestBuilder — main component
// ---------------------------------------------------------------------------

export default function RequestBuilder() {
  const { draft, sending, sendError, updateDraft, sendDraftRequest, cancelDraftRequest } = useRequestStore();
  const { activeEnvironmentId } = useEnvStore();
  const { updateRequest } = useCollectionStore();
  const [activeTab, setActiveTab] = useState<TabId>("params");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savedNotice, setSavedNotice] = useState(false);

  // Derive the set of currently valid variable keys
  const validKeys = useValidVariableSet();

  // -------------------------------------------------------------------------
  // URL ↔ Params sync handlers
  // -------------------------------------------------------------------------

  /** Called when the user types in the URL bar. */
  const handleUrlChange = useCallback((url: string) => {
    // 1. Sync query params: parse from new URL, preserve enabled state
    const newParams = parseQueryString(url, draft.params);

    // 2. Sync path params: detect :param tokens, preserve existing values
    const detected = extractPathParams(url);
    const prevMap = new Map(draft.pathParams.map((p) => [p.name, p.value]));
    const newPathParams = detected.map((name) => ({
      name,
      value: prevMap.get(name) ?? "",
    }));

    updateDraft({ url, params: newParams, pathParams: newPathParams });
  }, [draft.params, draft.pathParams, updateDraft]);

  /** Called when the Params table rows change. */
  const handleParamsChange = useCallback((params: QueryParam[]) => {
    // Rebuild query string in URL to stay in sync
    const updatedUrl = replaceQueryString(draft.url, params);
    updateDraft({ params, url: updatedUrl });
  }, [draft.url, updateDraft]);

  const handleSend = () => {
    sendDraftRequest(activeEnvironmentId);
  };

  const handleCancel = () => {
    cancelDraftRequest();
  };

  // Save the current draft back to the collection (only if it has an id).
  const handleSave = useCallback(async () => {
    if (!draft.id) return;
    setSaving(true);
    setSaveError(null);
    try {
      await updateRequest(
        draft.id,
        draft.name,
        draft.method,
        draft.url,
        draft.headers,
        draft.params,
        draft.body,
        draft.auth
      );
      setSavedNotice(true);
      setTimeout(() => setSavedNotice(false), 2000);
    } catch (err) {
      setSaveError(String(err));
    } finally {
      setSaving(false);
    }
  }, [draft, updateRequest]);

  const hasUrl = draft.url.trim().length > 0;
  const isSaved = !!draft.id;

  const enabledHeaderCount = draft.headers.filter((h) => h.enabled && h.key.trim()).length;
  const enabledParamCount = draft.params.filter((p) => p.enabled && p.key.trim()).length;
  const pathParamCount = draft.pathParams.length;

  return (
    <div className="app-main" aria-label="Request builder">
      {/* ------------------------------------------------------------------- */}
      {/* URL bar — 16.1                                                        */}
      {/* ------------------------------------------------------------------- */}
      <div className="url-bar">
        {/* Method selector */}
        <select
          className="url-bar__method env-selector__select"
          value={draft.method}
          onChange={(e) =>
            updateDraft({ method: e.target.value as typeof draft.method })
          }
          aria-label="HTTP method"
        >
          {(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] as const).map(
            (m) => (
              <option key={m} value={m}>
                {m}
              </option>
            )
          )}
        </select>

        {/* URL input with variable highlighting */}
        <VariableHighlightInput
          value={draft.url}
          onChange={handleUrlChange}
          placeholder="https://api.example.com/:resource/:id?foo=bar&{{variable}}"
          validKeys={validKeys}
        />

        {/* Send / Cancel button */}
        {sending ? (
          <button
            className="btn btn--danger url-bar__send"
            onClick={handleCancel}
            aria-label="Cancel request"
          >
            Cancel
          </button>
        ) : (
          <button
            className="btn btn--primary url-bar__send"
            onClick={handleSend}
            disabled={!hasUrl}
            aria-busy={false}
          >
            Send
          </button>
        )}

        {/* Save button — only shown when a saved request is loaded */}
        {isSaved && (
          <button
            className="btn btn--secondary url-bar__save"
            onClick={handleSave}
            disabled={saving}
            aria-busy={saving}
            title="Save changes to collection"
          >
            {saving ? "Saving…" : savedNotice ? "✓ Saved" : "Save"}
          </button>
        )}
      </div>

      {/* Error messages */}
      {sendError && (
        <p role="alert" className="send-error">
          {sendError}
        </p>
      )}
      {saveError && (
        <p role="alert" className="send-error">
          Save failed: {saveError}
        </p>
      )}

      {/* ------------------------------------------------------------------- */}
      {/* Tab panels — 16.2                                                    */}
      {/* ------------------------------------------------------------------- */}
      <TabBar
        active={activeTab}
        onSelect={setActiveTab}
        headerCount={enabledHeaderCount}
        paramCount={enabledParamCount + pathParamCount}
      />

      <div className="tab-content" role="tabpanel" aria-label={`${activeTab} panel`}>
        {/* Params tab — path variables (auto-detected) + query params */}
        {activeTab === "params" && (
          <div className="params-tab">
            {/* Path variables section — only shown when URL has :param tokens */}
            <PathParamsTable
              pathParams={draft.pathParams}
              onChange={(pathParams) => updateDraft({ pathParams })}
            />

            {/* Query params section */}
            {draft.pathParams.length > 0 && (
              <div className="params-tab__section-header">
                <span>Query Parameters</span>
              </div>
            )}
            <KeyValueTable<QueryParam>
              rows={draft.params}
              onChange={handleParamsChange}
              keyPlaceholder="Parameter"
              valuePlaceholder="Value"
              ariaLabel="Query parameters"
              validKeys={validKeys}
            />
          </div>
        )}

        {/* Headers tab */}
        {activeTab === "headers" && (
          <KeyValueTable<Header>
            rows={draft.headers}
            onChange={(headers) => updateDraft({ headers })}
            keyPlaceholder="Header name"
            valuePlaceholder="Value"
            ariaLabel="Request headers"
            validKeys={validKeys}
          />
        )}

        {/* Body tab */}
        {activeTab === "body" && (
          <BodyEditor
            body={draft.body}
            onChange={(body) => updateDraft({ body })}
            validKeys={validKeys}
          />
        )}

        {/* Auth tab */}
        {activeTab === "auth" && (
          <AuthEditor
            auth={draft.auth}
            onChange={(auth) => updateDraft({ auth })}
            showInherit={true}
          />
        )}
      </div>
    </div>
  );
}
