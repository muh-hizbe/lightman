# Design Document

## Overview

Lightman adalah aplikasi desktop API client berbasis Tauri 2.0. Backend ditulis dalam Rust dan menangani semua logika bisnis (eksekusi HTTP, persistensi SQLite, interpolasi variabel, manajemen autentikasi). Frontend ditulis dalam React/TypeScript dan bertanggung jawab atas presentasi UI dan state management menggunakan Zustand. Komunikasi antara frontend dan backend terjadi secara eksklusif melalui Tauri IPC (`invoke`).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Frontend (React/TypeScript)               │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Collections │  │   Request    │  │  Response        │  │
│  │  Sidebar     │  │   Builder    │  │  Viewer          │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Environment │  │   History    │  │  Import/Export   │  │
│  │  Manager     │  │   Panel      │  │  UI              │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│                   Zustand Stores                            │
│  (collectionStore, requestStore, envStore, historyStore)    │
│                                                             │
│              Tauri IPC  (invoke)                            │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                    Backend (Rust / Tauri)                    │
│                                                             │
│  Commands Layer (src/commands/)                             │
│  ┌───────────┐ ┌───────────┐ ┌──────────┐ ┌────────────┐  │
│  │collection │ │environment│ │   http   │ │  history   │  │
│  └───────────┘ └───────────┘ └──────────┘ └────────────┘  │
│  ┌───────────┐ ┌───────────┐ ┌──────────┐                  │
│  │  export   │ │  import   │ │  sample  │                  │
│  └───────────┘ └───────────┘ └──────────┘                  │
│                                                             │
│  Services Layer (src/services/)                             │
│  ┌───────────┐ ┌───────────────┐ ┌──────────────────────┐  │
│  │collection │ │ http_executor │ │  var_interpolator    │  │
│  └───────────┘ └───────────────┘ └──────────────────────┘  │
│  ┌───────────┐ ┌───────────────┐ ┌──────────────────────┐  │
│  │environment│ │ header_merger │ │  history             │  │
│  └───────────┘ └───────────────┘ └──────────────────────┘  │
│  ┌───────────┐ ┌───────────────┐                           │
│  │  export   │ │    import     │                           │
│  └───────────┘ └───────────────┘                           │
│                                                             │
│  Storage Layer (src/storage/)                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │   SQLite via rusqlite (WAL mode, bundled)            │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Architecture Decisions

- **Layered architecture**: Commands → Services → Storage. Commands are thin wrappers that validate inputs and delegate to services. Services contain business logic. Storage is a pure data access layer.
- **Async-first**: All Tauri commands are `async`. SQLite operations use `tokio::task::spawn_blocking` to avoid blocking the async executor.
- **No cloud dependency**: All data resides in `$APP_DATA_DIR/lightman.db`. No network calls are made except for user-initiated HTTP requests.
- **IPC boundary is typed**: All data crossing the Tauri IPC boundary uses `serde`-serializable structs.

## Components and Interfaces

### Commands Layer (`src/commands/`)

Each module exposes `#[tauri::command]` functions. Commands handle input validation, call services, and serialize responses.

| Module | Responsibility |
|---|---|
| `collection.rs` | Create, read, update, delete, duplicate Collections and Requests |
| `environment.rs` | Create, read, update, delete Environments and Variables |
| `http.rs` | Execute a single HTTP request (integrates interpolation, header merger, executor) |
| `history.rs` | List, search, delete HistoryEntries |
| `export.rs` | Serialize Collections to JSON and write via file dialog |
| `import.rs` | Read Postman/Lightman JSON files via file dialog and persist |
| `sample.rs` | Load built-in sample requests |

### Services Layer (`src/services/`)

| Module | Responsibility |
|---|---|
| `collection.rs` | CRUD business logic for Collections and nested Requests/folders |
| `http_executor.rs` | Build `reqwest::Client` and execute HTTP calls; returns `HttpResponse` |
| `var_interpolator.rs` | Resolve `{{var}}` placeholders using priority chain: env → collection → global |
| `header_merger.rs` | Merge collection default headers, request headers, and auth-injected headers |
| `environment.rs` | CRUD for Environments and Variables |
| `history.rs` | Persist and retrieve HistoryEntry records |
| `export.rs` | Serialize domain models to portable JSON |
| `import.rs` | Parse Postman v2.1 and Lightman JSON into domain models |
| `sample.rs` | Return static preset Request configurations |

### Storage Layer (`src/storage/`)

A single module `mod.rs` that:
- Opens (or creates) the SQLite database at `$APP_DATA_DIR/lightman.db`
- Enables WAL mode immediately after opening
- Runs schema migrations in version order
- Provides typed CRUD functions used by the services layer
- Wraps blocking rusqlite calls in `tokio::task::spawn_blocking`

## Data Models (`src/models/mod.rs`)

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Top-level grouping of requests, optionally containing sub-folders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,           // max 255 chars
    pub description: Option<String>,
    pub parent_id: Option<Uuid>, // None = root collection
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single HTTP request configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub name: String,           // max 255 chars
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<Header>,
    pub query_params: Vec<QueryParam>,
    pub body: RequestBody,
    pub auth: AuthConfig,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HttpMethod {
    Get, Post, Put, Patch, Delete, Head, Options,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    None,
    Json { content: String },
    FormUrlEncoded { fields: Vec<FormField> },
    Multipart { parts: Vec<MultipartPart> },
    Raw { content: String, content_type: String },
    Binary { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartPart {
    pub name: String,
    pub value: MultipartValue,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MultipartValue {
    Text { content: String },
    File { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    None,
    BasicAuth { username: String, password: String },
    BearerToken { token: String },
    ApiKey { key: String, value: String, location: ApiKeyLocation },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    Header,
    QueryParam,
}

/// A named set of variables representing one deployment context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: Uuid,
    pub name: String,           // max 255 chars
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single key-value variable within an Environment or the global scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub id: Uuid,
    pub environment_id: Option<Uuid>, // None = global variable
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

/// A record of one completed HTTP request execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub request_config: Request,   // stored as JSON blob
    pub status_code: u16,
    pub response_time_ms: u64,
    pub response_size_bytes: u64,
    pub response_headers: Vec<Header>,
    pub response_body: String,
    pub executed_at: DateTime<Utc>,
}

/// The result returned to the frontend after executing an HTTP request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<Header>,
    pub body: String,
    pub response_time_ms: u64,
    pub response_size_bytes: u64,
}
```

## Data Models

See the struct definitions above in the Components and Interfaces section.

## Database Schema

```sql
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL
);

-- Collections and folders (self-referential for nested folders)
CREATE TABLE IF NOT EXISTS collections (
    id          TEXT PRIMARY KEY,   -- UUID v4
    name        TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    description TEXT,
    parent_id   TEXT REFERENCES collections(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- HTTP request configurations (stored as JSON blob for body/headers/auth flexibility)
CREATE TABLE IF NOT EXISTS requests (
    id              TEXT PRIMARY KEY,
    collection_id   TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    name            TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    method          TEXT NOT NULL,
    url             TEXT NOT NULL,
    headers_json    TEXT NOT NULL DEFAULT '[]',
    params_json     TEXT NOT NULL DEFAULT '[]',
    body_json       TEXT NOT NULL DEFAULT '{"type":"none"}',
    auth_json       TEXT NOT NULL DEFAULT '{"type":"none"}',
    position        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- Environments
CREATE TABLE IF NOT EXISTS environments (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 255),
    is_active   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Variables (environment-scoped or global when environment_id IS NULL)
CREATE TABLE IF NOT EXISTS variables (
    id              TEXT PRIMARY KEY,
    environment_id  TEXT REFERENCES environments(id) ON DELETE CASCADE,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL DEFAULT '',
    enabled         INTEGER NOT NULL DEFAULT 1
);

-- Request execution history
CREATE TABLE IF NOT EXISTS history (
    id                  TEXT PRIMARY KEY,
    request_json        TEXT NOT NULL,      -- serialized Request struct
    status_code         INTEGER NOT NULL,
    response_time_ms    INTEGER NOT NULL,
    response_size_bytes INTEGER NOT NULL,
    response_headers_json TEXT NOT NULL DEFAULT '[]',
    response_body       TEXT NOT NULL DEFAULT '',
    executed_at         TEXT NOT NULL
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_requests_collection ON requests(collection_id);
CREATE INDEX IF NOT EXISTS idx_variables_env       ON variables(environment_id);
CREATE INDEX IF NOT EXISTS idx_history_executed    ON history(executed_at DESC);
```

## Service Interfaces

### HTTP Executor

```rust
pub struct HttpExecutorService;

impl HttpExecutorService {
    /// Executes a fully-resolved Request (all {{vars}} already replaced).
    /// Returns HttpResponse on success or a descriptive error string on failure.
    pub async fn execute(request: &Request) -> Result<HttpResponse, String>;
}
```

### Variable Interpolator

```rust
pub struct VarInterpolatorService;

impl VarInterpolatorService {
    /// Replaces all {{key}} occurrences in `text` using the priority chain:
    ///   env_vars (highest) → collection_vars → global_vars (lowest).
    /// Unresolved placeholders are left unchanged.
    pub fn interpolate(
        text: &str,
        env_vars: &[Variable],
        collection_vars: &[Variable],
        global_vars: &[Variable],
    ) -> String;
}
```

### Header Merger

```rust
pub struct HeaderMergerService;

impl HeaderMergerService {
    /// Merges collection-level default headers and request-level headers,
    /// then appends auth headers derived from `auth_config`.
    /// Auth headers override any manually-set header with the same name.
    pub fn merge(
        collection_defaults: &[Header],
        request_headers: &[Header],
        auth_config: &AuthConfig,
    ) -> Vec<Header>;
}
```

## Variable Interpolation Priority

When resolving `{{variable_name}}`:

1. **Active Environment Variables** — highest priority
2. **Collection Variables** — mid-level (future extension point)
3. **Global Variables** — lowest priority (no `environment_id`)
4. **Unresolved** — placeholder left as-is in the output string

Interpolation is applied to: URL, all enabled header values, query parameter values, and request body text fields.

## Import Format: Postman Collection v2.1

The import service accepts JSON conforming to the Postman Collection Format v2.1 schema. Key mapping:

| Postman field | Lightman model |
|---|---|
| `info.name` | `Collection.name` |
| `item[]` (with `item` children) | Nested folder → `Collection` with `parent_id` |
| `item[]` (with `request`) | `Request` |
| `request.method` | `Request.method` |
| `request.url.raw` | `Request.url` |
| `request.header[]` | `Request.headers` |
| `request.body` | `Request.body` |
| `request.auth` | `Request.auth` |

## Export Format: Lightman JSON

```json
{
  "lightman_version": "1",
  "exported_at": "<ISO 8601 timestamp>",
  "collection": { "<Collection struct>" },
  "requests": [ "<Request structs>" ]
}
```

## Error Handling

All Tauri Commands return `Result<T, String>`. The `String` error type carries a human-readable message. The frontend receives this as a rejected promise and displays it in an error notification.

Error categories:
- **Validation errors**: Invalid input from the frontend (e.g., empty collection name).
- **Storage errors**: SQLite failures, migrations failures.
- **HTTP errors**: Network failure, DNS resolution failure, timeout, TLS errors.
- **Import errors**: File not found, malformed JSON, unsupported Postman version.
- **Serialization errors**: Unexpected `serde_json` failures.

All errors are logged to the Tauri logger before being returned to the frontend.

## Frontend State Management (Zustand)

```typescript
// Collection store — mirrors backend collection/request tree
interface CollectionStore {
  collections: Collection[];
  fetchCollections: () => Promise<void>;
  createCollection: (name: string) => Promise<void>;
  deleteCollection: (id: string) => Promise<void>;
}

// Active request store — tracks the currently-edited request
interface RequestStore {
  activeRequest: Request | null;
  response: HttpResponse | null;
  isLoading: boolean;
  sendRequest: () => Promise<void>;
}

// Environment store
interface EnvStore {
  environments: Environment[];
  activeEnvId: string | null;
  setActiveEnv: (id: string) => void;
}

// History store
interface HistoryStore {
  entries: HistoryEntry[];
  fetchHistory: () => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
}
```

## Testing Strategy

### Unit Tests

Unit tests menggunakan modul `#[cfg(test)]` inline Rust, fokus pada:
- Serialisasi/deserialisasi model (serde round-trip)
- Validasi input di services layer
- Edge case untuk `VarInterpolatorService` (string kosong, nested placeholders, karakter khusus)
- Header merge logic termasuk override dan conflict resolution

### Property-Based Tests (`tests/property/`)

Property-based tests menggunakan `proptest` dan berjalan dengan minimum 100 iterasi per property. Setiap test direferensikan ke property yang didefinisikan di bagian Correctness Properties.

- `prop_name_constraints.rs` — Properties 1
- `prop_persistence.rs` — Properties 2, 3
- `prop_http_execution.rs` — Properties 4, 8, 9 (menggunakan `wiremock` untuk mock server)
- `prop_variable_resolution.rs` — Properties 5, 6, 7
- `prop_export_import.rs` — Properties 10, 11, 12
- `prop_auth.rs` — Properties 13, 14
- `prop_inherited_headers.rs` — Properties 15, 16
- `prop_samples.rs` — Property 17

### Integration Tests (`tests/integration/`)

Integration tests menggunakan `wiremock` dan SQLite in-memory untuk menguji end-to-end flows tanpa side effects:
- `test_send_request_e2e.rs` — full request pipeline (interpolation → merge → execute)
- `test_export_import_file.rs` — file-based import/export round trip
- `test_startup_loading.rs` — database initialization dan migrations
- `test_performance_smoke.rs` — smoke test untuk verifikasi latensi baseline

### Frontend Tests

Frontend menggunakan Vitest dengan `@testing-library/react` untuk:
- Unit test Zustand stores (mocked Tauri `invoke`)
- Component test untuk request builder dan response viewer
- Snapshot test untuk layout kritis

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Name Length Constraint

For any Collection, Request, or Environment name, the Storage Layer SHALL reject names with length less than 1 or greater than 255 characters.

**Validates: Requirements 2.1, 2.2, 3.1**

### Property 2: Request Config Round-Trip Persistence

For any valid Request configuration, serializing the Request to JSON and then deserializing the stored JSON SHALL produce a value equal to the original Request.

**Validates: Requirements 2.3, 7.1**

### Property 3: Recursive Delete Completeness

For any Collection with arbitrarily nested sub-folders and Requests, deleting the root Collection SHALL result in zero remaining Requests or sub-folders referencing that Collection in the Storage Layer.

**Validates: Requirements 2.5, 7.4**

### Property 4: HTTP Method Round-Trip

For any HttpMethod value, serializing it to a string and parsing it back SHALL produce a value equal to the original HttpMethod.

**Validates: Requirements 1.1**

### Property 5: Interpolation Identity on Placeholder-Free Strings

For any string containing no `{{` or `}}` characters, the Variable Interpolator SHALL return the string unchanged regardless of the variable sets provided.

**Validates: Requirements 3.2**

### Property 6: Interpolation Completeness for Present Variables

For any string containing `{{key}}` and any non-empty variable map that contains `key`, the Variable Interpolator SHALL return a string containing no `{{key}}` placeholder.

**Validates: Requirements 3.2**

### Property 7: Interpolation Preserves Unresolved Placeholders

For any string containing `{{key}}` where `key` is absent from all variable scopes, the Variable Interpolator SHALL return a string that still contains `{{key}}` unchanged.

**Validates: Requirements 3.5**

### Property 8: HTTP Executor Returns Status Code for Any Valid Request

For any well-formed HTTP request sent to a reachable mock server, the HTTP Executor SHALL return an HttpResponse whose `status_code` matches the status code returned by the server.

**Validates: Requirements 1.2, 1.5**

### Property 9: Response Payload Integrity

For any response body returned by a mock server, the HTTP Executor's returned `body` string SHALL equal the UTF-8 decoded response body from the server.

**Validates: Requirements 1.5**

### Property 10: Export-Import Round Trip for Collections

For any Collection and its Requests, exporting to Lightman JSON and then importing the resulting JSON SHALL produce Collections and Requests with identical configurations (excluding generated timestamps and UUIDs).

**Validates: Requirements 6.3**

### Property 11: Import Idempotency on Error

For any invalid import file, calling the import command SHALL leave the Storage Layer in the same state as before the call (no partial records written).

**Validates: Requirements 6.2, 7.4**

### Property 12: Export Produces Valid JSON

For any non-empty Collection, the export service SHALL produce a string that is valid JSON and contains the `lightman_version` and `collection` keys.

**Validates: Requirements 6.3**

### Property 13: Basic Auth Header Encoding

For any non-empty username and password pair, the Header Merger SHALL generate an `Authorization` header whose value equals `Basic ` followed by the standard base64 encoding of `username:password`.

**Validates: Requirements 9.2**

### Property 14: Bearer Token Header Injection

For any non-empty bearer token string, the Header Merger SHALL generate an `Authorization` header whose value equals `Bearer <token>`.

**Validates: Requirements 9.3**

### Property 15: Auth Header Overrides Manual Header

For any request that has a manually-set `Authorization` header and also has an auth config that generates an `Authorization` header, the Header Merger's output SHALL contain exactly one `Authorization` header with the value generated by the auth config.

**Validates: Requirements 9.6**

### Property 16: Merge Preserves Non-Conflicting Headers

For any set of collection default headers and request headers with no overlapping keys and no auth config, the Header Merger's output SHALL contain all headers from both sets with their original values.

**Validates: Requirements 9.6**

### Property 17: Sample Requests Are Well-Formed

For any sample request returned by the sample service, the Request SHALL have a non-empty URL, a valid HttpMethod, and all fields present and deserializable.

**Validates: Requirements 10.1, 10.3**
