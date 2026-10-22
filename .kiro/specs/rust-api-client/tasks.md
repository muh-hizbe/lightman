# Implementation Plan: Lightman — Rust API Client

## Overview

Implementasi Lightman dilakukan secara incremental: setup proyek → data layer → services layer → commands → frontend shell → frontend fitur → tests. Setiap task membangun di atas task sebelumnya, diakhiri dengan wiring semua komponen bersama.

## Tasks

- [x] 1. Scaffold proyek Tauri 2.0 dan setup dependensi
  - Setup `Cargo.toml` dengan semua dependensi (tauri 2, reqwest, rusqlite, serde, uuid, chrono, base64, proptest, wiremock)
  - Setup `package.json` dengan React 19, TypeScript, Zustand, Tauri JS API
  - Buat struktur direktori `src-tauri/src/` (commands/, services/, models/, storage/)
  - Buat stub file untuk semua module (mod.rs, placeholder comments)
  - Konfigurasi Tauri permissions di `capabilities/default.json`
  - _Requirements: 7.1, 7.4, 8.1, 8.5_

- [x] 2. Implementasi database schema dan storage layer
  - [x] 2.1 Implementasi inisialisasi database dan migrations di `storage/mod.rs`
    - Buka atau buat SQLite database di App Data Directory
    - Aktifkan WAL mode (`PRAGMA journal_mode=WAL`)
    - Buat tabel `schema_migrations` untuk version tracking
    - Jalankan migration v1: buat semua tabel (collections, requests, environments, variables, history)
    - Tambahkan indexes untuk query patterns umum
    - _Requirements: 7.1, 7.2, 7.3, 7.6_
  - [x] 2.2 Implementasi CRUD functions untuk collections dan requests di `storage/mod.rs`
    - `insert_collection`, `get_collection`, `list_collections`, `update_collection`, `delete_collection` (cascade)
    - `insert_request`, `get_request`, `list_requests_by_collection`, `update_request`, `delete_request`
    - Semua operasi SQLite dibungkus dalam `tokio::task::spawn_blocking`
    - _Requirements: 2.1, 2.2, 2.3, 2.5, 7.4_
  - [x] 2.3 Implementasi CRUD functions untuk environments, variables, dan history di `storage/mod.rs`
    - `insert_environment`, `get_active_environment`, `list_environments`, `update_environment`, `delete_environment`
    - `insert_variable`, `list_variables_by_env`, `delete_variables_by_env`
    - `insert_history_entry`, `list_history`, `delete_history_entry`, `clear_history`
    - _Requirements: 3.1, 3.3, 4.1, 4.2, 4.5, 7.1_
  - [x] 2.4 Tulis property test untuk request config round-trip dan recursive delete
    - **Property 2: Request Config Round-Trip Persistence**
    - **Property 3: Recursive Delete Completeness**
    - **Validates: Requirements 2.3, 2.5, 7.1**
    - File: `tests/property/prop_persistence.rs`

- [x] 3. Implementasi domain models (`src/models/mod.rs`)
  - [x] 3.1 Definisi semua struct domain di `models/mod.rs`
    - `Collection`, `Request`, `HttpMethod`, `Header`, `QueryParam`
    - `RequestBody` (enum: None, Json, FormUrlEncoded, Multipart, Raw, Binary)
    - `AuthConfig` (enum: None, BasicAuth, BearerToken, ApiKey), `ApiKeyLocation`
    - `Environment`, `Variable`, `HistoryEntry`, `HttpResponse`
    - Semua struct derive `Serialize`, `Deserialize`, `Debug`, `Clone`
    - _Requirements: 1.1, 1.3, 2.1, 3.1, 4.1, 9.1_
  - [x] 3.2 Tulis property test untuk name length constraints
    - **Property 1: Name Length Constraint**
    - **Validates: Requirements 2.1, 2.2, 3.1**
    - File: `tests/property/prop_name_constraints.rs`

- [x] 4. Implementasi HTTP Executor service (`src/services/http_executor.rs`)
  - [x] 4.1 Implementasi `HttpExecutorService::execute` menggunakan reqwest
    - Build `reqwest::Client` dengan cookie store
    - Routing berdasarkan `HttpMethod` ke metode reqwest yang sesuai
    - Attach headers, query params dari `Request` struct
    - Attach body berdasarkan `RequestBody` variant (json, form, multipart, raw, binary)
    - Ukur response time menggunakan `std::time::Instant`
    - Return `HttpResponse` dengan status, headers, body, time, size
    - _Requirements: 1.1, 1.2, 1.3, 1.5_
  - [x] 4.2 Implementasi error handling untuk network dan timeout errors
    - Map reqwest errors ke error string yang deskriptif
    - Tangani timeout, DNS failure, TLS errors
    - _Requirements: 1.6, 1.7_
  - [x] 4.3 Tulis unit test untuk HTTP method routing
    - Test bahwa setiap `HttpMethod` dipetakan ke reqwest method yang benar
    - _Requirements: 1.1_
  - [x] 4.4 Tulis property test untuk HTTP executor
    - **Property 4: HTTP Method Round-Trip**
    - **Property 8: HTTP Executor Returns Status Code for Any Valid Request**
    - **Property 9: Response Payload Integrity**
    - **Validates: Requirements 1.1, 1.2, 1.5**
    - Gunakan `wiremock` sebagai mock server
    - File: `tests/property/prop_http_execution.rs`

- [x] 5. Implementasi Variable Interpolator service (`src/services/var_interpolator.rs`)
  - [x] 5.1 Implementasi `VarInterpolatorService::interpolate`
    - Parse string untuk mencari pola `{{key}}`
    - Resolve key menggunakan priority chain: env_vars → collection_vars → global_vars
    - Biarkan placeholder tidak berubah jika key tidak ditemukan di semua scope
    - Apply interpolation ke URL, enabled header values, query param values, body text fields
    - _Requirements: 3.2, 3.4, 3.5_
  - [x] 5.2 Tulis property test untuk variable interpolation
    - **Property 5: Interpolation Identity on Placeholder-Free Strings**
    - **Property 6: Interpolation Completeness for Present Variables**
    - **Property 7: Interpolation Preserves Unresolved Placeholders**
    - **Validates: Requirements 3.2, 3.5**
    - File: `tests/property/prop_variable_resolution.rs`

- [x] 6. Implementasi Header Merger service (`src/services/header_merger.rs`)
  - [x] 6.1 Implementasi `HeaderMergerService::merge`
    - Merge collection default headers + request headers (request overrides collection)
    - Inject auth headers berdasarkan `AuthConfig`:
      - `BasicAuth`: base64-encode `username:password`, set `Authorization: Basic <encoded>`
      - `BearerToken`: set `Authorization: Bearer <token>`
      - `ApiKey` as header: inject key-value sebagai request header
      - `ApiKey` as query param: append ke URL query string
    - Auth headers override header manual yang sama namanya
    - _Requirements: 9.2, 9.3, 9.4, 9.5, 9.6_
  - [x] 6.2 Tulis property test untuk auth header generation
    - **Property 13: Basic Auth Header Encoding**
    - **Property 14: Bearer Token Header Injection**
    - **Validates: Requirements 9.2, 9.3**
    - File: `tests/property/prop_auth.rs`
  - [x] 6.3 Tulis property test untuk header merge behavior
    - **Property 15: Auth Header Overrides Manual Header**
    - **Property 16: Merge Preserves Non-Conflicting Headers**
    - **Validates: Requirements 9.6**
    - File: `tests/property/prop_inherited_headers.rs`

- [x] 7. Implementasi Collection service dan commands
  - [x] 7.1 Implementasi `services/collection.rs`
    - `create_collection`, `get_collection`, `list_collections`, `update_collection`, `delete_collection`
    - `create_request`, `get_request`, `list_requests`, `update_request`, `delete_request`, `duplicate_request`
    - `reorder_request` (update position index)
    - Validasi: nama tidak boleh kosong, panjang max 255 karakter
    - _Requirements: 2.1, 2.2, 2.4, 2.6, 2.7_
  - [x] 7.2 Implementasi `commands/collection.rs` sebagai Tauri commands
    - Expose semua service functions sebagai `#[tauri::command]` async functions
    - Serialize return values ke serde-compatible types
    - _Requirements: 2.1, 2.2, 8.3_

- [x] 8. Implementasi Environment service dan commands
  - [x] 8.1 Implementasi `services/environment.rs`
    - `create_environment`, `list_environments`, `update_environment`, `delete_environment`
    - `set_active_environment` (deactivate semua lalu activate satu)
    - `create_variable`, `list_variables`, `update_variable`, `delete_variable`
    - `list_global_variables` (variables dengan `environment_id = NULL`)
    - _Requirements: 3.1, 3.3, 3.6_
  - [x] 8.2 Implementasi `commands/environment.rs` sebagai Tauri commands
    - Expose semua environment dan variable operations
    - _Requirements: 3.1, 3.6, 8.3_

- [x] 9. Implementasi HTTP command — send request (`src/commands/http.rs`)
  - [x] 9.1 Implementasi `commands/http.rs::send_request`
    - Terima `Request` struct dan active environment ID dari frontend
    - Ambil active environment variables dan global variables dari storage
    - Apply `VarInterpolatorService::interpolate` ke URL, headers, params, body
    - Apply `HeaderMergerService::merge` dengan collection defaults dan auth config
    - Panggil `HttpExecutorService::execute` dengan resolved request
    - Simpan `HistoryEntry` ke storage setelah eksekusi
    - Return `HttpResponse` ke frontend
    - _Requirements: 1.2, 1.4, 1.5, 3.2, 9.1, 4.1_
  - [x] 9.2 Tulis integration test untuk full request pipeline
    - Test end-to-end: interpolation → merge → execute → history persistence
    - File: `tests/integration/test_send_request_e2e.rs`

- [x] 10. Implementasi History service dan commands
  - [x] 10.1 Implementasi `services/history.rs`
    - `save_entry`, `list_entries` (ordered by executed_at DESC)
    - `search_entries` (filter by URL substring atau HTTP method)
    - `delete_entry`, `clear_all`
    - _Requirements: 4.1, 4.2, 4.3, 4.5_
  - [x] 10.2 Implementasi `commands/history.rs` sebagai Tauri commands
    - Expose history operations ke frontend
    - _Requirements: 4.2, 4.3, 4.5, 8.3_

- [x] 11. Implementasi Import/Export service dan commands
  - [x] 11.1 Implementasi `services/export.rs`
    - Serialize `Collection` beserta semua `Request`-nya ke Lightman JSON format
    - Sertakan `lightman_version`, `exported_at`, `collection`, `requests` keys
    - _Requirements: 6.3_
  - [x] 11.2 Implementasi `services/import.rs` untuk Postman Collection v2.1
    - Parse JSON, validasi field `info.schema` untuk versi v2.1
    - Map Postman `item[]` (dengan sub-items) ke nested Collections
    - Map Postman `item[]` (dengan `request`) ke `Request` structs
    - Persist ke storage dalam satu transaksi; rollback pada error
    - _Requirements: 6.1, 6.2, 7.4_
  - [x] 11.3 Implementasi `services/import.rs` untuk Lightman JSON format
    - Parse dan validasi Lightman JSON
    - Persist Collections dan Requests
    - _Requirements: 6.3_
  - [x] 11.4 Implementasi `commands/export.rs` dan `commands/import.rs`
    - Export: panggil Tauri dialog `save` untuk memilih path, tulis JSON ke file
    - Import: panggil Tauri dialog `open` untuk memilih file, baca dan parse
    - _Requirements: 6.4, 8.3_
  - [x] 11.5 Tulis property test untuk export-import round trip
    - **Property 10: Export-Import Round Trip for Collections**
    - **Property 11: Import Idempotency on Error**
    - **Property 12: Export Produces Valid JSON**
    - **Validates: Requirements 6.2, 6.3**
    - File: `tests/property/prop_export_import.rs`

- [x] 12. Implementasi Sample Requests service dan commands
  - [x] 12.1 Implementasi `services/sample.rs`
    - Return static list `Request` preset: GET JSONPlaceholder (`/todos/1`), POST JSONPlaceholder (`/todos`), GET httpbin.org (`/get`), POST httpbin.org (`/post`)
    - Semua sample memiliki UUID statis yang deterministik, URL yang valid, dan HttpMethod yang valid
    - _Requirements: 10.1, 10.3_
  - [x] 12.2 Implementasi `commands/sample.rs` sebagai Tauri command
    - Expose `list_samples` command
    - _Requirements: 10.2, 10.4, 8.3_
  - [x] 12.3 Tulis property test untuk sample request integrity
    - **Property 17: Sample Requests Are Well-Formed**
    - **Validates: Requirements 10.1, 10.3**
    - File: `tests/property/prop_samples.rs`

- [x] 13. Checkpoint — Pastikan semua tests backend lulus
  - Ensure all tests pass, ask the user if questions arise.

- [x] 14. Wire semua commands ke Tauri invoke_handler (`src/lib.rs`)
  - [x] 14.1 Update `lib.rs` untuk register semua Tauri commands
    - Import semua command modules
    - Tambahkan semua command functions ke `tauri::generate_handler![]`
    - Hapus placeholder `greet` command
    - _Requirements: 8.4_

- [x] 15. Implementasi React frontend — layout utama dan Zustand stores
  - [x] 15.1 Buat struktur direktori frontend dan Zustand stores
    - Buat `src/stores/` dengan `collectionStore.ts`, `requestStore.ts`, `envStore.ts`, `historyStore.ts`
    - Definisi TypeScript types yang mirror Rust models (`Collection`, `Request`, `HttpResponse`, `Environment`, `Variable`, `HistoryEntry`)
    - Setup Tauri `invoke` wrappers di `src/api/` untuk setiap command
    - _Requirements: 8.2, 8.3_
  - [x] 15.2 Implementasi App shell layout di `src/App.tsx`
    - Tiga-panel layout: sidebar kiri (collections), panel tengah (request builder), panel kanan (response viewer)
    - Toolbar atas dengan environment selector
    - Responsive CSS layout
    - _Requirements: 8.2_

- [x] 16. Implementasi frontend — request builder
  - [x] 16.1 Implementasi komponen URL bar dan method selector
    - Method dropdown (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
    - URL input dengan variable highlighting (`{{var}}` ditandai secara visual)
    - Send button yang trigger `requestStore.sendRequest()`
    - _Requirements: 1.1, 3.4_
  - [x] 16.2 Implementasi tab panels di request builder
    - Tab "Headers": tambah/hapus/toggle key-value header rows
    - Tab "Params": tambah/hapus/toggle query parameter rows
    - Tab "Body": pilih body type (None, JSON, Form, Multipart, Raw, Binary)
    - Body editor sesuai tipe (textarea untuk JSON/Raw, form table untuk Form/Multipart)
    - Tab "Auth": pilih auth type, input fields sesuai auth type
    - _Requirements: 1.3, 9.1_

- [x] 17. Implementasi frontend — response viewer
  - [x] 17.1 Implementasi response status bar
    - Tampilkan status code dengan color coding (2xx hijau, 4xx oranye, 5xx merah)
    - Tampilkan response time (ms) dan response size (bytes)
    - _Requirements: 5.1, 5.2_
  - [x] 17.2 Implementasi response body viewer
    - Tab "Pretty": JSON pretty-print dengan syntax highlighting (jika body valid JSON)
    - Tab "Raw": display body sebagai plain text tanpa formatting
    - Fallback ke raw jika body bukan JSON
    - _Requirements: 5.3, 5.4, 5.5_
  - [x] 17.3 Implementasi response headers viewer
    - Tabel key-value untuk response headers
    - _Requirements: 5.6_

- [x] 18. Implementasi frontend — collections sidebar
  - [x] 18.1 Implementasi tree view untuk collections
    - Recursive render `Collection` nodes dan `Request` leaf nodes
    - Expand/collapse folder nodes
    - Klik request → load ke request builder via `requestStore`
    - _Requirements: 2.4_
  - [x] 18.2 Implementasi CRUD UI untuk collections
    - Context menu / toolbar buttons: buat collection, buat folder, buat request, rename, delete, duplicate
    - Inline rename input
    - _Requirements: 2.1, 2.2, 2.6_

- [x] 19. Implementasi frontend — environment manager
  - [x] 19.1 Implementasi environment selector dropdown di toolbar
    - List semua environments, highlight yang aktif
    - Klik environment → panggil `set_active_environment` command
    - _Requirements: 3.6_
  - [x] 19.2 Implementasi environment editor modal/panel
    - CRUD environments
    - Tabel variables (key, value, enabled toggle)
    - Bagian terpisah untuk Global Variables
    - _Requirements: 3.1, 3.3_

- [x] 20. Implementasi frontend — history panel
  - [x] 20.1 Implementasi history list view
    - Tampilkan list HistoryEntries: method, URL, status code, timestamp
    - Sort descending by timestamp
    - _Requirements: 4.2_
  - [x] 20.2 Implementasi history search dan re-send
    - Input search untuk filter by URL atau method
    - Klik entry → load request config ke request builder
    - Tombol delete per entry dan "Clear All"
    - _Requirements: 4.3, 4.4, 4.5_

- [x] 21. Implementasi frontend — import/export UI
  - [x] 21.1 Implementasi tombol import di collections sidebar
    - Tombol "Import" membuka file dialog (filter .json)
    - Panggil `import_collection` command, refresh collection list setelah sukses
    - Tampilkan error notification jika import gagal
    - _Requirements: 6.1, 6.2, 6.4_
  - [x] 21.2 Implementasi tombol export di collections sidebar
    - Context menu "Export" pada collection node
    - Buka save file dialog, panggil `export_collection` command
    - _Requirements: 6.3, 6.4_

- [x] 22. Checkpoint — Pastikan semua fitur frontend terhubung dan integration tests lulus
  - Ensure all tests pass, ask the user if questions arise.

- [x] 23. Tulis integration tests end-to-end
  - [x] 23.1 Integration test untuk full request pipeline
    - Test: variable interpolation → header merge → HTTP execute → history save
    - File: `tests/integration/test_send_request_e2e.rs`
    - _Requirements: 1.2, 1.4, 3.2, 4.1_
  - [x] 23.2 Integration test untuk export-import file
    - Test: create collection → export ke temp file → import dari file → compare
    - File: `tests/integration/test_export_import_file.rs`
    - _Requirements: 6.1, 6.3_
  - [x] 23.3 Integration test untuk startup loading
    - Test: database initialization, migration idempotency
    - File: `tests/integration/test_startup_loading.rs`
    - _Requirements: 7.2, 7.3, 7.6_

## Notes

- Tasks bertanda `*` adalah opsional dan bisa dilewati untuk MVP yang lebih cepat
- Setiap task mereferensikan requirement spesifik untuk traceability
- Checkpoint memastikan validasi inkremental
- Property tests memvalidasi correctness properties universal (minimum 100 iterasi per test)
- Unit tests memvalidasi contoh spesifik dan edge cases

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["2.1"] },
    { "id": 1, "tasks": ["2.2", "2.3", "3.1"] },
    { "id": 2, "tasks": ["2.4", "3.2", "4.1", "5.1", "6.1"] },
    { "id": 3, "tasks": ["4.2", "5.2", "6.2", "6.3", "7.1", "8.1"] },
    { "id": 4, "tasks": ["4.3", "4.4", "7.2", "8.2", "11.1", "11.2", "11.3", "12.1"] },
    { "id": 5, "tasks": ["9.1", "10.1", "11.4", "12.2"] },
    { "id": 6, "tasks": ["9.2", "10.2", "11.5", "12.3", "14.1"] },
    { "id": 7, "tasks": ["15.1"] },
    { "id": 8, "tasks": ["15.2"] },
    { "id": 9, "tasks": ["16.1", "17.1", "18.1", "19.1", "20.1"] },
    { "id": 10, "tasks": ["16.2", "17.2", "17.3", "18.2", "19.2", "20.2", "21.1", "21.2"] },
    { "id": 11, "tasks": ["23.1", "23.2", "23.3"] }
  ]
}
```
