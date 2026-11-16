// Shared variable-highlighting input components.
// Used in RequestBuilder (auth, headers, params, body) and Sidebar (collection auth modal).

import { useMemo } from "react";
import { useEnvStore } from "../stores/envStore";

// ---------------------------------------------------------------------------
// useValidVariableSet — derives the set of valid variable keys.
// ---------------------------------------------------------------------------

export function useValidVariableSet(): Set<string> {
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
// buildHighlightedHtml — replaces {{key}} with coloured <mark> tags.
// ---------------------------------------------------------------------------

export function buildHighlightedHtml(
  text: string,
  validKeys: Set<string>,
): string {
  const escaped = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  return escaped.replace(/\{\{([^}]*)\}\}/g, (_match, key: string) => {
    const cls = validKeys.has(key) ? "var-valid" : "var-invalid";
    return `<mark class="var-highlight ${cls}">{{${key}}}</mark>`;
  });
}

// ---------------------------------------------------------------------------
// VarHighlightInlineInput — overlay input with {{var}} highlighting.
// ---------------------------------------------------------------------------

export function VarHighlightInlineInput({
  value,
  onChange,
  placeholder,
  ariaLabel,
  validKeys,
  inputType = "text",
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  validKeys: Set<string>;
  /** Use "password" to mask the actual text while still showing highlights. */
  inputType?: "text" | "password";
}) {
  // For password fields, only show highlights when the field has content —
  // the highlight layer is hidden via opacity when inputType is "password"
  // so the browser's masking dots are what the user sees; the highlight
  // layer kicks in only on focus-away (we keep it simple: always hide it
  // for password type so the mask dots aren't double-rendered).
  const highlighted =
    inputType === "password" ? "" : buildHighlightedHtml(value, validKeys);

  if (inputType === "password") {
    // Plain password input — no overlay trick needed (browser masks it anyway)
    return (
      <input
        className="input kv-table__input"
        type="password"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={ariaLabel}
        autoComplete="new-password"
        spellCheck={false}
      />
    );
  }

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
// VarHighlightFormInput — same overlay trick but styled as a form field
// (uses `.input` without `.kv-table__input`, for use inside form-group).
// ---------------------------------------------------------------------------

export function VarHighlightFormInput({
  id,
  value,
  onChange,
  placeholder,
  ariaLabel,
  validKeys,
  inputType = "text",
}: {
  id?: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  validKeys: Set<string>;
  inputType?: "text" | "password";
}) {
  if (inputType === "password") {
    return (
      <input
        id={id}
        className="input"
        type="password"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={ariaLabel}
        autoComplete="new-password"
        spellCheck={false}
      />
    );
  }

  const highlighted = buildHighlightedHtml(value, validKeys);

  return (
    <div className="var-input-wrapper">
      <div
        className="var-input-highlight"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: highlighted || "" }}
      />
      <input
        id={id}
        className="var-input-real input"
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
