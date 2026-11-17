// CodePreview — generates cURL, axios, and fetch snippets from the current
// draft request. Env variables and path params are resolved to their values.

import { useState, useMemo } from "react";
import type { DraftRequest } from "../stores/requestStore";
import { substitutePathParams } from "../stores/requestStore";
import type { AuthConfig, RequestBody } from "../types";
import { useEnvStore } from "../stores/envStore";
import { useCollectionStore } from "../stores/collectionStore";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type SnippetLang = "curl" | "axios" | "fetch";

// ---------------------------------------------------------------------------
// Variable interpolation
// ---------------------------------------------------------------------------

/**
 * Replace every {{key}} in `text` with its value from `varMap`.
 * Keys with no entry are left as {{key}} so the snippet stays readable.
 */
function interpolate(text: string, varMap: Map<string, string>): string {
  return text.replace(/\{\{([^}]+)\}\}/g, (_m, key: string) => varMap.get(key) ?? `{{${key}}}`);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build the final URL string (path params substituted, query string appended, api_key-in-query added). */
function buildUrl(draft: DraftRequest, varMap: Map<string, string>): string {
  // substitutePathParams resolves :param tokens; interpolate resolves {{var}} tokens
  let url = interpolate(substitutePathParams(draft.url, draft.pathParams), varMap);
  const enabled = draft.params.filter((p) => p.enabled && p.key.trim());

  // Append api_key-in-query param if applicable
  if (draft.auth.type === "api_key" && draft.auth.location === "query_param" && draft.auth.key) {
    enabled.push({ key: draft.auth.key, value: draft.auth.value, enabled: true });
  }

  if (enabled.length > 0) {
    const qs = enabled
      .map((p) => {
        const k = interpolate(p.key, varMap);
        const v = interpolate(p.value, varMap);
        return v ? `${k}=${v}` : k;
      })
      .join("&");
    const hasQ = url.includes("?");
    url += (hasQ ? "&" : "?") + qs;
  }
  return url || "https://example.com";
}

/**
 * Collect every header the request will send, with values interpolated:
 * - collection default_headers (already resolved from inherit chain)
 * - explicit enabled headers from the draft (override defaults with same key)
 * - Content-Type derived from the body type
 * - Authorization / API key headers from resolvedAuth
 */
function collectHeaders(
  draft: DraftRequest,
  varMap: Map<string, string>,
  resolvedAuth: AuthConfig,
  collectionDefaultHeaders: { key: string; value: string; enabled: boolean }[],
): { key: string; value: string }[] {
  const result: { key: string; value: string }[] = [];

  // Collection default headers first (lower priority)
  for (const h of collectionDefaultHeaders) {
    if (h.enabled && h.key.trim()) {
      result.push({
        key: interpolate(h.key, varMap),
        value: interpolate(h.value, varMap),
      });
    }
  }

  // User-defined headers on the request (overwrite any default with same key)
  for (const h of draft.headers) {
    if (h.enabled && h.key.trim()) {
      const key = interpolate(h.key, varMap);
      const value = interpolate(h.value, varMap);
      const existing = result.findIndex((r) => r.key.toLowerCase() === key.toLowerCase());
      if (existing !== -1) {
        result[existing] = { key, value };
      } else {
        result.push({ key, value });
      }
    }
  }

  // Content-Type from body (only add when not already present)
  const ct = deriveContentType(draft.body);
  if (ct) {
    const alreadySet = result.some((h) => h.key.toLowerCase() === "content-type");
    if (!alreadySet) result.push({ key: "Content-Type", value: ct });
  }

  // Auth headers — use the already-resolved auth (inherit chain walked in component)
  const auth = resolvedAuth;
  if (auth.type === "bearer_token") {
    const token = interpolate(auth.token, varMap);
    result.push({ key: "Authorization", value: `Bearer ${token}` });
  } else if (auth.type === "basic_auth") {
    const user = interpolate(auth.username, varMap);
    const pass = interpolate(auth.password, varMap);
    const encoded = btoa(`${user}:${pass}`);
    result.push({ key: "Authorization", value: `Basic ${encoded}` });
  } else if (auth.type === "api_key" && auth.location === "header") {
    const key = interpolate(auth.key, varMap);
    const value = interpolate(auth.value, varMap);
    result.push({ key, value });
  }

  return result;
}


function deriveContentType(body: RequestBody): string | null {
  switch (body.type) {
    case "json":
      return "application/json";
    case "raw":
      return body.content_type || "text/plain";
    case "form_url_encoded":
      return "application/x-www-form-urlencoded";
    case "multipart":
      return null; // curl/fetch set boundary automatically
    default:
      return null;
  }
}

/** Return the body string representation for a snippet, with values interpolated. */
function bodyToString(body: RequestBody, varMap: Map<string, string>): string | null {
  switch (body.type) {
    case "none":
      return null;
    case "json":
      return interpolate(body.content.trim(), varMap) || null;
    case "raw":
      return interpolate(body.content.trim(), varMap) || null;
    case "form_url_encoded": {
      const fields = body.fields
        .filter((f) => f.enabled && f.key.trim())
        .map((f) => {
          const k = encodeURIComponent(interpolate(f.key, varMap));
          const v = encodeURIComponent(interpolate(f.value, varMap));
          return `${k}=${v}`;
        })
        .join("&");
      return fields || null;
    }
    case "multipart": {
      const lines = body.fields
        .filter((f) => f.enabled && f.key.trim())
        .map((f) => `  -F '${interpolate(f.key, varMap)}=${interpolate(f.value, varMap)}'`)
        .join(" \\\n");
      return lines || null;
    }
    case "binary":
      return body.file_path ? `@${body.file_path}` : null;
    default:
      return null;
  }
}

// ---------------------------------------------------------------------------
// Snippet generators
// ---------------------------------------------------------------------------

function generateCurl(
  draft: DraftRequest,
  varMap: Map<string, string>,
  resolvedAuth: AuthConfig,
  collectionDefaultHeaders: { key: string; value: string; enabled: boolean }[],
): string {
  const url = buildUrl(draft, varMap);
  const headers = collectHeaders(draft, varMap, resolvedAuth, collectionDefaultHeaders);
  const lines: string[] = [`curl -X ${draft.method} '${url}'`];

  for (const h of headers) {
    lines.push(`  -H '${h.key}: ${h.value}'`);
  }

  if (draft.body.type === "multipart") {
    const fields = draft.body.fields.filter((f) => f.enabled && f.key.trim());
    for (const f of fields) {
      lines.push(`  -F '${interpolate(f.key, varMap)}=${interpolate(f.value, varMap)}'`);
    }
  } else {
    const bodyStr = bodyToString(draft.body, varMap);
    if (bodyStr) {
      if (draft.body.type === "binary") {
        lines.push(`  --data-binary '${bodyStr}'`);
      } else {
        const escaped = bodyStr.replace(/'/g, "'\\''");
        lines.push(`  --data '${escaped}'`);
      }
    }
  }

  return lines.join(" \\\n");
}

function generateFetch(
  draft: DraftRequest,
  varMap: Map<string, string>,
  resolvedAuth: AuthConfig,
  collectionDefaultHeaders: { key: string; value: string; enabled: boolean }[],
): string {
  const url = buildUrl(draft, varMap);
  const headers = collectHeaders(draft, varMap, resolvedAuth, collectionDefaultHeaders);
  const bodyStr = draft.body.type !== "multipart" ? bodyToString(draft.body, varMap) : null;

  const opts: string[] = [];
  opts.push(`  method: '${draft.method}'`);

  if (headers.length > 0) {
    const hLines = headers
      .map((h) => `    '${h.key}': '${h.value}'`)
      .join(",\n");
    opts.push(`  headers: {\n${hLines}\n  }`);
  }

  if (draft.body.type === "multipart") {
    opts.push(`  // body: formData  // append fields to a FormData object`);
  } else if (bodyStr) {
    const escaped = bodyStr.replace(/`/g, "\\`");
    opts.push(`  body: \`${escaped}\``);
  }

  return (
    `fetch('${url}', {\n` +
    opts.join(",\n") +
    "\n})\n" +
    "  .then((res) => res.json())\n" +
    "  .then((data) => console.log(data))\n" +
    "  .catch((err) => console.error(err));"
  );
}

function generateAxios(
  draft: DraftRequest,
  varMap: Map<string, string>,
  resolvedAuth: AuthConfig,
  collectionDefaultHeaders: { key: string; value: string; enabled: boolean }[],
): string {
  const url = buildUrl(draft, varMap);
  const headers = collectHeaders(draft, varMap, resolvedAuth, collectionDefaultHeaders);
  const bodyStr = draft.body.type !== "multipart" ? bodyToString(draft.body, varMap) : null;

  const opts: string[] = [];

  if (headers.length > 0) {
    const hLines = headers
      .map((h) => `    '${h.key}': '${h.value}'`)
      .join(",\n");
    opts.push(`  headers: {\n${hLines}\n  }`);
  }

  if (draft.body.type === "multipart") {
    opts.push(`  // data: formData  // append fields to a FormData object`);
  } else if (bodyStr) {
    let dataStr: string;
    if (draft.body.type === "json") {
      try {
        const parsed = JSON.parse(bodyStr);
        dataStr = `  data: ${JSON.stringify(parsed, null, 4).replace(/\n/g, "\n  ")}`;
      } catch {
        dataStr = `  data: \`${bodyStr.replace(/`/g, "\\`")}\``;
      }
    } else {
      dataStr = `  data: \`${bodyStr.replace(/`/g, "\\`")}\``;
    }
    opts.push(dataStr);
  }

  const method = draft.method.toLowerCase();
  const hasOpts = opts.length > 0;

  if (hasOpts) {
    return (
      `axios({\n` +
      `  method: '${method}',\n` +
      `  url: '${url}',\n` +
      opts.join(",\n") +
      "\n})\n" +
      "  .then((res) => console.log(res.data))\n" +
      "  .catch((err) => console.error(err));"
    );
  }

  return (
    `axios.${method}('${url}')\n` +
    "  .then((res) => console.log(res.data))\n" +
    "  .catch((err) => console.error(err));"
  );
}

// ---------------------------------------------------------------------------
// Syntax tokeniser — very lightweight, no deps
// ---------------------------------------------------------------------------

/** Convert a plain code string into an HTML string with syntax spans. */
function highlightCode(code: string, lang: SnippetLang): string {
  if (lang === "curl") return highlightCurl(code);
  return highlightJs(code);
}

function esc(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function highlightCurl(code: string): string {
  // Tokenise line-by-line to keep it simple.
  // Flags are highlighted BEFORE quoted strings so the regex only runs on
  // unquoted text — this avoids matching `-bf` inside a UUID in a URL.
  return code
    .split("\n")
    .map((line) => {
      // Split the line into quoted-string tokens and unquoted tokens, then
      // only apply flag highlighting to the unquoted portions.
      const parts = line.split(/(\'[^\']*\')/g);
      const processed = parts.map((part, idx) => {
        // Odd indices are the captured quoted strings — wrap and leave alone
        if (idx % 2 === 1) {
          return `<span class="cp-string">${esc(part)}</span>`;
        }
        // Even indices are unquoted text — apply flag + keyword highlighting
        let out = part.replace(
          // Flags must be preceded by whitespace or start-of-string, and
          // must not be followed by more non-space chars (i.e. they stand alone)
          /(^|\s)(--?[a-zA-Z][a-zA-Z-]*)(?=\s|$)/g,
          (_m, sp, flag) => `${sp}<span class="cp-flag">${esc(flag)}</span>`
        );
        // curl keyword
        out = out.replace(/^(\s*)(curl)\b/, (_m, sp, kw) => `${sp}<span class="cp-keyword">${kw}</span>`);
        // line continuation backslash at end of unquoted segment
        out = out.replace(/\\$/, '<span class="cp-punct">\\</span>');
        return out;
      });
      return processed.join("");
    })
    .join("\n");
}

function highlightJs(code: string): string {
  // Keywords
  const keywords = new Set([
    "const", "let", "var", "function", "return", "if", "else",
    "new", "true", "false", "null", "undefined", "async", "await",
    "then", "catch",
  ]);

  // We tokenise roughly: strings first, then identifiers, then numbers, then punctuation
  const tokens: { type: string; raw: string }[] = [];
  let i = 0;
  const len = code.length;

  while (i < len) {
    // Template literal
    if (code[i] === "`") {
      let s = "`";
      i++;
      while (i < len && code[i] !== "`") {
        if (code[i] === "\\") { s += code[i] + (code[i + 1] ?? ""); i += 2; }
        else s += code[i++];
      }
      s += "`"; i++;
      tokens.push({ type: "string", raw: s });
      continue;
    }
    // Single-quoted string
    if (code[i] === "'") {
      let s = "'";
      i++;
      while (i < len && code[i] !== "'") {
        if (code[i] === "\\") { s += code[i] + (code[i + 1] ?? ""); i += 2; }
        else s += code[i++];
      }
      s += "'"; i++;
      tokens.push({ type: "string", raw: s });
      continue;
    }
    // Double-quoted string
    if (code[i] === '"') {
      let s = '"';
      i++;
      while (i < len && code[i] !== '"') {
        if (code[i] === "\\") { s += code[i] + (code[i + 1] ?? ""); i += 2; }
        else s += code[i++];
      }
      s += '"'; i++;
      tokens.push({ type: "string", raw: s });
      continue;
    }
    // Comment
    if (code[i] === "/" && code[i + 1] === "/") {
      let s = "";
      while (i < len && code[i] !== "\n") s += code[i++];
      tokens.push({ type: "comment", raw: s });
      continue;
    }
    // Identifier / keyword
    if (/[a-zA-Z_$]/.test(code[i])) {
      let s = "";
      while (i < len && /[a-zA-Z0-9_$]/.test(code[i])) s += code[i++];
      tokens.push({ type: keywords.has(s) ? "keyword" : "ident", raw: s });
      continue;
    }
    // Number
    if (/[0-9]/.test(code[i])) {
      let s = "";
      while (i < len && /[0-9.]/.test(code[i])) s += code[i++];
      tokens.push({ type: "number", raw: s });
      continue;
    }
    // Punctuation / operator
    if (/[{}()[\]:,;=.<>!+\-*/&|^~%?@\\]/.test(code[i])) {
      tokens.push({ type: "punct", raw: code[i++] });
      continue;
    }
    // Whitespace / newlines — pass through
    tokens.push({ type: "ws", raw: code[i++] });
  }

  return tokens
    .map(({ type, raw }) => {
      const e = esc(raw);
      switch (type) {
        case "keyword": return `<span class="cp-keyword">${e}</span>`;
        case "string":  return `<span class="cp-string">${e}</span>`;
        case "number":  return `<span class="cp-number">${e}</span>`;
        case "comment": return `<span class="cp-comment">${e}</span>`;
        case "punct":   return `<span class="cp-punct">${e}</span>`;
        default:        return e;
      }
    })
    .join("");
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function CodePreview({ draft }: { draft: DraftRequest }) {
  const [lang, setLang] = useState<SnippetLang>("curl");
  const [copied, setCopied] = useState(false);

  // Build variable map: global vars first, active env vars override
  const { activeEnvironmentId, variablesByEnv, globalVariables } = useEnvStore();
  const varMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const v of globalVariables) {
      if (v.enabled) map.set(v.key, v.value);
    }
    if (activeEnvironmentId) {
      const envVars = variablesByEnv[activeEnvironmentId] ?? [];
      for (const v of envVars) {
        if (v.enabled) map.set(v.key, v.value);
      }
    }
    return map;
  }, [activeEnvironmentId, variablesByEnv, globalVariables]);

  // Resolve inherit chain: walk up collection ancestors until we hit a
  // concrete auth (not "inherit"), or fall back to "none".
  const { collections } = useCollectionStore();
  const { resolvedAuth, collectionDefaultHeaders } = useMemo(() => {
    // Build a map for fast lookup
    const colMap = new Map(collections.map((c) => [c.id, c]));

    // Walk the ancestor chain starting from the request's own collection
    let auth: AuthConfig = draft.auth;
    let collId: string | null = draft.collection_id;

    // Accumulate default_headers from each ancestor (child overrides parent for same key)
    // We collect from outermost ancestor inward so child takes priority.
    const ancestorChain: typeof collections = [];
    while (collId) {
      const col = colMap.get(collId);
      if (!col) break;
      ancestorChain.unshift(col); // prepend so root is first
      collId = col.parent_id;
    }

    // Merge default_headers: root → ... → direct parent, child wins on same key
    const mergedDefaultHeaders = new Map<string, { key: string; value: string; enabled: boolean }>();
    for (const col of ancestorChain) {
      for (const h of col.default_headers) {
        if (h.enabled && h.key.trim()) {
          mergedDefaultHeaders.set(h.key.toLowerCase(), h);
        }
      }
    }

    // Resolve auth: if request auth is "inherit", walk ancestors for first non-inherit
    if (auth.type === "inherit") {
      auth = { type: "none" }; // fallback
      // Walk from direct parent upward
      for (let i = ancestorChain.length - 1; i >= 0; i--) {
        const col = ancestorChain[i];
        if (col.auth.type !== "inherit") {
          auth = col.auth;
          break;
        }
      }
    }

    return {
      resolvedAuth: auth,
      collectionDefaultHeaders: [...mergedDefaultHeaders.values()],
    };
  }, [draft.auth, draft.collection_id, collections]);

  const code = useMemo(() => {
    switch (lang) {
      case "curl":  return generateCurl(draft, varMap, resolvedAuth, collectionDefaultHeaders);
      case "fetch": return generateFetch(draft, varMap, resolvedAuth, collectionDefaultHeaders);
      case "axios": return generateAxios(draft, varMap, resolvedAuth, collectionDefaultHeaders);
    }
  }, [lang, draft, varMap, resolvedAuth, collectionDefaultHeaders]);

  const highlighted = useMemo(() => highlightCode(code, lang), [code, lang]);

  const handleCopy = () => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div className="code-preview">
      <div className="code-preview__toolbar">
        <div className="code-preview__langs" role="group" aria-label="Snippet language">
          {(["curl", "fetch", "axios"] as SnippetLang[]).map((l) => (
            <button
              key={l}
              className={`btn btn--ghost code-preview__lang-btn${lang === l ? " code-preview__lang-btn--active" : ""}`}
              onClick={() => setLang(l)}
              aria-pressed={lang === l}
            >
              {l}
            </button>
          ))}
        </div>
        <button
          className="btn btn--ghost btn--sm code-preview__copy"
          onClick={handleCopy}
          aria-label="Copy snippet to clipboard"
        >
          {copied ? "✓ Copied" : "Copy"}
        </button>
      </div>

      <pre
        className="code-preview__pre"
        aria-label={`${lang} snippet`}
        dangerouslySetInnerHTML={{ __html: highlighted }}
      />
    </div>
  );
}
