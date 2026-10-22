# Requirements Document

## Introduction

Lightman adalah aplikasi desktop API client yang ringan dan mengutamakan privasi, dibangun dengan Tauri 2.0 (Rust backend) dan React/TypeScript frontend. Lightman menyediakan antarmuka untuk membuat dan mengirim HTTP request, mengelola koleksi request, mengelola environment variables, melihat riwayat request, dan mengimpor/mengekspor koleksi — semua tersimpan secara lokal tanpa cloud sync.

## Glossary

- **Lightman**: Aplikasi desktop API client yang menjadi subjek spesifikasi ini.
- **Collection**: Kumpulan HTTP request yang dikelompokkan dan dapat berisi sub-folder.
- **Request**: Konfigurasi lengkap satu HTTP call, termasuk metode, URL, headers, body, dan autentikasi.
- **Environment**: Satu set named variables (key-value pairs) yang dapat diaktifkan secara global dan diinterpolasi ke dalam request.
- **Variable**: Pasangan key-value yang didefinisikan dalam suatu Environment atau sebagai Global Variable.
- **HistoryEntry**: Rekaman satu eksekusi HTTP request beserta response-nya.
- **Interpolation**: Proses penggantian placeholder `{{variable_name}}` dengan nilai variabel yang aktif saat runtime.
- **Storage Layer**: Lapisan persistensi berbasis SQLite yang bertanggung jawab atas semua operasi baca/tulis data.
- **HTTP Executor**: Komponen Rust yang membangun dan mengirimkan HTTP request menggunakan reqwest.
- **Variable Interpolator**: Komponen Rust yang menyelesaikan referensi `{{variable_name}}` dalam sebuah string.
- **Header Merger**: Komponen Rust yang menggabungkan default headers dengan request headers dan menyuntikkan auth headers.
- **Tauri Command**: Fungsi Rust yang diekspos ke frontend melalui mekanisme IPC Tauri.
- **App Data Directory**: Direktori data aplikasi yang dikelola oleh sistem operasi, digunakan sebagai lokasi penyimpanan database SQLite.

## Requirements

### Requirement 1: HTTP Request Execution

**User Story:** As a developer, I want to build and send HTTP requests with full control over method, URL, headers, query parameters, and body, so that I can test and debug any REST API.

#### Acceptance Criteria

1. THE Lightman SHALL support HTTP methods GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS.
2. WHEN a user sends a request, THE HTTP Executor SHALL construct and dispatch the request using the reqwest library.
3. WHEN a user sets a request body, THE Lightman SHALL support body types: JSON, URL-encoded form data, multipart/form-data, raw text, and binary file upload.
4. WHEN a request URL contains `{{variable_name}}` placeholders, THE Variable Interpolator SHALL resolve each placeholder before the request is sent.
5. WHEN a request is sent successfully, THE Lightman SHALL capture and return the response status code, headers, body, response time in milliseconds, and response size in bytes.
6. IF an HTTP request fails due to a network error, THEN THE HTTP Executor SHALL return a structured error describing the failure reason.
7. IF a request timeout occurs, THEN THE HTTP Executor SHALL return a timeout error after the configured duration.

### Requirement 2: Collections Management

**User Story:** As a developer, I want to organize my HTTP requests into named collections and folders, so that I can maintain a structured and reusable library of API calls.

#### Acceptance Criteria

1. THE Lightman SHALL allow a user to create, read, update, and delete Collections.
2. THE Lightman SHALL allow a user to create, read, update, and delete Requests within a Collection.
3. WHEN a user creates a Request inside a Collection, THE Storage Layer SHALL persist the Request with a unique UUID and the Collection's identifier.
4. THE Lightman SHALL support nested folders within a Collection up to arbitrary depth.
5. WHEN a user deletes a Collection, THE Storage Layer SHALL recursively delete all child folders and Requests within that Collection.
6. WHEN a user duplicates a Request, THE Lightman SHALL create a new Request with a new UUID and the same configuration as the original.
7. THE Lightman SHALL allow a user to reorder Requests and folders within a Collection using a position index.

### Requirement 3: Environment Variables

**User Story:** As a developer, I want to define multiple environments (e.g., dev, staging, production) with their own variables, so that I can switch contexts without modifying my request configurations.

#### Acceptance Criteria

1. THE Lightman SHALL allow a user to create, read, update, and delete Environments.
2. WHEN an Environment is active, THE Variable Interpolator SHALL resolve `{{variable_name}}` placeholders using the active Environment's variables first, then Collection-level variables, then Global Variables.
3. THE Lightman SHALL support a set of Global Variables that are available across all Environments.
4. WHEN a user updates a variable value in the active Environment, THE Lightman SHALL apply the new value to all subsequent requests without requiring a restart.
5. WHEN a `{{variable_name}}` reference cannot be resolved in any variable scope, THE Variable Interpolator SHALL leave the placeholder unchanged in the final request string.
6. THE Lightman SHALL allow a user to set one Environment as active at a time; setting a new active Environment SHALL deactivate the previously active one.

### Requirement 4: Request History

**User Story:** As a developer, I want a persistent log of all requests I've sent, so that I can review past calls, reproduce issues, and re-send previous requests.

#### Acceptance Criteria

1. WHEN a request is sent, THE Storage Layer SHALL persist a HistoryEntry containing the full request configuration and the response metadata (status code, response time, response size).
2. THE Lightman SHALL allow a user to list all HistoryEntries, ordered by timestamp descending.
3. WHEN a user searches the history, THE Lightman SHALL filter HistoryEntries by URL substring or HTTP method.
4. WHEN a user selects a HistoryEntry, THE Lightman SHALL populate the request builder with the stored request configuration.
5. THE Lightman SHALL allow a user to delete individual HistoryEntries or clear the entire history.
6. THE Storage Layer SHALL store history entries in the SQLite database in the App Data Directory.

### Requirement 5: Response Viewer

**User Story:** As a developer, I want to view response details clearly formatted, so that I can quickly understand API responses without manual parsing.

#### Acceptance Criteria

1. WHEN a response is received, THE Lightman SHALL display the HTTP status code and status text.
2. WHEN a response is received, THE Lightman SHALL display total response time in milliseconds and response body size in bytes.
3. WHEN a response body is valid JSON, THE Lightman SHALL render the body as syntax-highlighted, indented JSON in a "Pretty" view.
4. WHEN a response body is not valid JSON, THE Lightman SHALL display the body as raw text.
5. THE Lightman SHALL provide a "Raw" view that displays the response body without any formatting.
6. THE Lightman SHALL display all response headers as a key-value list.

### Requirement 6: Import and Export

**User Story:** As a developer, I want to import collections from Postman and export my collections to a portable format, so that I can migrate existing work and share configurations.

#### Acceptance Criteria

1. WHEN a user imports a file in Postman Collection v2.1 JSON format, THE Lightman SHALL parse the file and create corresponding Collections and Requests in the Storage Layer.
2. IF the imported file is not a valid Postman Collection v2.1 document, THEN THE Lightman SHALL return a structured import error with a description of the validation failure.
3. WHEN a user exports a Collection, THE Lightman SHALL write a Lightman JSON file containing all Requests and folder structure of that Collection.
4. WHEN a user initiates an import or export, THE Lightman SHALL use the system file dialog (via Tauri dialog plugin) to select the source or destination file path.

### Requirement 7: Local Storage and Persistence

**User Story:** As a privacy-conscious developer, I want all my data stored locally on my machine, so that my API configurations and credentials never leave my device.

#### Acceptance Criteria

1. THE Storage Layer SHALL store all data (Collections, Requests, Environments, Variables, HistoryEntries) exclusively in a SQLite database file located in the App Data Directory.
2. THE Storage Layer SHALL initialize the SQLite database with WAL (Write-Ahead Logging) journal mode on first launch to improve concurrent read performance.
3. WHEN the application starts, THE Storage Layer SHALL apply all pending database schema migrations in version order before accepting any other operations.
4. IF a database write operation fails, THEN THE Storage Layer SHALL return a structured error and SHALL NOT partially commit the transaction.
5. THE Lightman SHALL NOT transmit any user data to external servers or cloud services.
6. WHEN the application starts, THE Storage Layer SHALL create the SQLite database file if it does not already exist.

### Requirement 8: Application Setup and Platform Integration

**User Story:** As a developer, I want a native desktop application that integrates with my operating system, so that I can use it alongside my other tools with familiar desktop behaviors.

#### Acceptance Criteria

1. THE Lightman SHALL be built as a Tauri 2.0 desktop application with a Rust backend and a React/TypeScript frontend.
2. WHEN the application window is shown, THE Lightman SHALL present a responsive layout with a sidebar for collections, a main panel for the request builder, and a response panel.
3. THE Lightman SHALL use Tauri IPC (`invoke`) for all communication between the frontend and the Rust backend.
4. THE Lightman SHALL register all Tauri Commands in the `invoke_handler` within `lib.rs` before the application window is presented.
5. WHERE the user's operating system requires file system access permissions, THE Lightman SHALL request only the permissions listed in `capabilities/default.json` (read file, write file, read dir, appdata scope).

### Requirement 9: Authentication Support

**User Story:** As a developer, I want to configure authentication for my requests using common auth schemes, so that I can test secured APIs without manually constructing auth headers.

#### Acceptance Criteria

1. THE Lightman SHALL support the following authentication types per Request: None, Basic Auth, Bearer Token, and API Key.
2. WHEN Basic Auth is configured, THE Header Merger SHALL generate an `Authorization: Basic <base64(username:password)>` header and inject it into the outgoing request.
3. WHEN Bearer Token auth is configured, THE Header Merger SHALL inject an `Authorization: Bearer <token>` header into the outgoing request.
4. WHEN API Key auth is configured as a header, THE Header Merger SHALL inject the key-value pair as a request header.
5. WHEN API Key auth is configured as a query parameter, THE Header Merger SHALL append the key-value pair to the request URL's query string.
6. WHEN auth headers are injected by THE Header Merger, the injected headers SHALL take precedence over any manually set headers with the same name.

### Requirement 10: Sample and Preset Requests

**User Story:** As a developer who is new to the application, I want a set of built-in sample requests, so that I can verify the application is working and explore its features without configuring anything.

#### Acceptance Criteria

1. THE Lightman SHALL provide a set of built-in sample requests targeting public APIs (JSONPlaceholder and httpbin.org).
2. WHEN a user loads sample requests, THE Lightman SHALL make them available as a read-only Collection in the collections list.
3. THE Lightman SHALL include at minimum the following sample request types: GET (retrieve resource), POST (create resource), and an httpbin.org echo request.
4. WHEN a user selects a sample request, THE Lightman SHALL populate the request builder with the sample's pre-configured method, URL, and body.
