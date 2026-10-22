// Import service — validates and loads Collection/Environment from JSON file.
// Uses all-or-nothing strategy: validates entire file before writing to storage.
//
// Task 11.2: Import Postman Collection v2.1 format.
// Task 11.3: Import Lightman JSON format (re-uses the export envelope).
//
// Requirements: 6.1, 6.2, 6.3, 7.4

use crate::models::{AuthConfig, ApiKeyLocation, Header, HttpMethod, QueryParam, RequestBody};
use crate::services::export::{CollectionNode, LightmanExport, LIGHTMAN_FORMAT_VERSION};
use crate::storage::DbHandle;
use crate::services::collection as collection_svc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Postman Collection v2.1 raw deserialization structs
// ---------------------------------------------------------------------------

/// The `info` block present at the top level of a Postman collection.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanInfo {
    /// Collection display name.
    name: String,
    /// Schema URL used to validate the collection version.
    /// Must contain "v2.1" to be accepted.
    schema: String,
    /// Optional free-text description (may be a string or object in the spec;
    /// we accept only the string variant here).
    #[serde(default)]
    description: Option<serde_json::Value>,
}

/// A Postman URL — can be a plain string or a structured object.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum PostmanUrl {
    /// Plain string URL (most common in v2.1 exports).
    Raw(String),
    /// Structured URL object.
    Object(PostmanUrlObject),
}

impl PostmanUrl {
    /// Resolve the URL to a plain string.
    fn to_string_url(&self) -> String {
        match self {
            PostmanUrl::Raw(s) => s.clone(),
            PostmanUrl::Object(obj) => {
                // Prefer the raw field if present.
                if let Some(raw) = &obj.raw {
                    return raw.clone();
                }
                // Reconstruct from protocol + host + path.
                let proto = obj
                    .protocol
                    .as_deref()
                    .map(|p| format!("{p}://"))
                    .unwrap_or_default();
                let host = obj
                    .host
                    .as_ref()
                    .map(|h| match h {
                        PostmanUrlHost::String(s) => s.clone(),
                        PostmanUrlHost::Array(parts) => parts.join("."),
                    })
                    .unwrap_or_default();
                let path = obj
                    .path
                    .as_ref()
                    .map(|p| match p {
                        PostmanUrlPath::String(s) => s.clone(),
                        PostmanUrlPath::Array(parts) => format!("/{}", parts.join("/")),
                    })
                    .unwrap_or_default();
                format!("{proto}{host}{path}")
            }
        }
    }

    /// Extract query params from a structured URL object (plain strings have none here).
    fn query_params(&self) -> Vec<QueryParam> {
        if let PostmanUrl::Object(obj) = self {
            if let Some(queries) = &obj.query {
                return queries
                    .iter()
                    .map(|q| QueryParam {
                        key: q.key.clone().unwrap_or_default(),
                        value: q.value.clone().unwrap_or_default(),
                        enabled: !q.disabled.unwrap_or(false),
                    })
                    .collect();
            }
        }
        vec![]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanUrlObject {
    raw: Option<String>,
    protocol: Option<String>,
    host: Option<PostmanUrlHost>,
    path: Option<PostmanUrlPath>,
    query: Option<Vec<PostmanQueryParam>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum PostmanUrlHost {
    String(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum PostmanUrlPath {
    String(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanQueryParam {
    key: Option<String>,
    value: Option<String>,
    disabled: Option<bool>,
    description: Option<serde_json::Value>,
}

/// A Postman header — either a key/value object or a plain string.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum PostmanHeader {
    Object(PostmanHeaderObject),
    Raw(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanHeaderObject {
    key: Option<String>,
    value: Option<String>,
    disabled: Option<bool>,
    description: Option<serde_json::Value>,
}

/// A Postman body payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanBody {
    /// Body mode: "raw", "urlencoded", "formdata", "file", "graphql".
    mode: Option<String>,
    raw: Option<String>,
    urlencoded: Option<Vec<PostmanFormField>>,
    formdata: Option<Vec<PostmanFormField>>,
    options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanFormField {
    key: Option<String>,
    value: Option<String>,
    disabled: Option<bool>,
    #[serde(rename = "type")]
    field_type: Option<String>,
}

/// Auth attribute key-value pair as used in the `auth` block.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanAuthAttr {
    key: String,
    value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    attr_type: Option<String>,
}

/// The `auth` block on a request or the root collection.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanAuth {
    #[serde(rename = "type")]
    auth_type: String,
    basic: Option<Vec<PostmanAuthAttr>>,
    bearer: Option<Vec<PostmanAuthAttr>>,
    apikey: Option<Vec<PostmanAuthAttr>>,
}

/// A Postman request definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanRequest {
    method: Option<String>,
    header: Option<Vec<PostmanHeader>>,
    url: Option<PostmanUrl>,
    body: Option<PostmanBody>,
    auth: Option<PostmanAuth>,
    description: Option<serde_json::Value>,
}

/// A Postman item (leaf = request) or item-group (folder).
/// Discriminated by the presence of `item` (sub-items) vs `request`.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanItem {
    name: Option<String>,
    /// Present when this is a leaf request item.
    request: Option<PostmanRequest>,
    /// Present when this is a folder (item-group).
    item: Option<Vec<PostmanItem>>,
    description: Option<serde_json::Value>,
}

/// Top-level Postman Collection v2.1 document.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    variable: Option<Vec<serde_json::Value>>,
    auth: Option<PostmanAuth>,
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

/// Map a Postman method string to `HttpMethod`, defaulting to GET.
fn map_method(method: Option<&str>) -> HttpMethod {
    match method.map(|m| m.to_uppercase()).as_deref() {
        Some("POST") => HttpMethod::Post,
        Some("PUT") => HttpMethod::Put,
        Some("PATCH") => HttpMethod::Patch,
        Some("DELETE") => HttpMethod::Delete,
        Some("HEAD") => HttpMethod::Head,
        Some("OPTIONS") => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

/// Map a list of Postman header values to `Vec<Header>`.
fn map_headers(headers: Option<&Vec<PostmanHeader>>) -> Vec<Header> {
    let Some(headers) = headers else {
        return vec![];
    };
    headers
        .iter()
        .filter_map(|h| match h {
            PostmanHeader::Object(obj) => {
                let key = obj.key.clone().unwrap_or_default();
                let value = obj.value.clone().unwrap_or_default();
                if key.is_empty() {
                    return None;
                }
                Some(Header {
                    key,
                    value,
                    enabled: !obj.disabled.unwrap_or(false),
                })
            }
            PostmanHeader::Raw(_) => None, // raw header strings are uncommon; skip
        })
        .collect()
}

/// Map a Postman body to `RequestBody`.
fn map_body(body: Option<&PostmanBody>) -> RequestBody {
    let Some(body) = body else {
        return RequestBody::None;
    };

    match body.mode.as_deref() {
        Some("raw") => {
            let content = body.raw.clone().unwrap_or_default();
            // Detect content type from options.raw.language if present.
            let language = body
                .options
                .as_ref()
                .and_then(|o| o.get("raw"))
                .and_then(|r| r.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("text");

            if language.eq_ignore_ascii_case("json") {
                RequestBody::Json { content }
            } else {
                let content_type = match language.to_lowercase().as_str() {
                    "html" => "text/html",
                    "xml" => "application/xml",
                    "javascript" => "application/javascript",
                    _ => "text/plain",
                };
                RequestBody::Raw {
                    content,
                    content_type: content_type.to_string(),
                }
            }
        }
        Some("urlencoded") => {
            let fields = body
                .urlencoded
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|f| crate::models::MultipartField {
                    key: f.key.clone().unwrap_or_default(),
                    value: f.value.clone().unwrap_or_default(),
                    enabled: !f.disabled.unwrap_or(false),
                })
                .collect();
            RequestBody::FormUrlEncoded { fields }
        }
        Some("formdata") => {
            let fields = body
                .formdata
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|f| crate::models::MultipartField {
                    key: f.key.clone().unwrap_or_default(),
                    value: f.value.clone().unwrap_or_default(),
                    enabled: !f.disabled.unwrap_or(false),
                })
                .collect();
            RequestBody::Multipart { fields }
        }
        Some("file") => RequestBody::Binary {
            file_path: String::new(), // file paths are local to Postman — leave blank
        },
        _ => RequestBody::None,
    }
}

/// Extract a string attribute value from a list of Postman auth attributes.
fn attr_str<'a>(attrs: &'a [PostmanAuthAttr], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_ref())
        .and_then(|v| v.as_str())
}

/// Map a Postman auth block to `AuthConfig`.
fn map_auth(auth: Option<&PostmanAuth>) -> AuthConfig {
    let Some(auth) = auth else {
        return AuthConfig::None;
    };

    match auth.auth_type.as_str() {
        "basic" => {
            let attrs = auth.basic.as_deref().unwrap_or(&[]);
            let username = attr_str(attrs, "username").unwrap_or("").to_string();
            let password = attr_str(attrs, "password").unwrap_or("").to_string();
            AuthConfig::BasicAuth { username, password }
        }
        "bearer" => {
            let attrs = auth.bearer.as_deref().unwrap_or(&[]);
            let token = attr_str(attrs, "token").unwrap_or("").to_string();
            AuthConfig::BearerToken { token }
        }
        "apikey" => {
            let attrs = auth.apikey.as_deref().unwrap_or(&[]);
            let key = attr_str(attrs, "key").unwrap_or("").to_string();
            let value = attr_str(attrs, "value").unwrap_or("").to_string();
            // "in" field can be "header" or "query"
            let location = match attr_str(attrs, "in") {
                Some("query") => ApiKeyLocation::QueryParam,
                _ => ApiKeyLocation::Header,
            };
            AuthConfig::ApiKey { key, value, location }
        }
        _ => AuthConfig::None, // noauth, oauth1, oauth2, etc. → no auth
    }
}

/// Sanitize a name: trim whitespace, truncate to 255 chars, fall back to a
/// default if empty.
fn sanitize_name(name: Option<&str>, fallback: &str) -> String {
    let name = name.unwrap_or("").trim();
    let name = if name.is_empty() { fallback } else { name };
    if name.len() > 255 {
        name[..255].to_string()
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// ImportService
// ---------------------------------------------------------------------------

pub struct ImportService;

impl ImportService {
    // -----------------------------------------------------------------------
    // Postman Collection v2.1
    // -----------------------------------------------------------------------

    /// Import a Postman Collection v2.1 JSON string into the database.
    ///
    /// Steps:
    /// 1. Parse and validate the JSON (checks `info.schema` for "v2.1").
    /// 2. Recursively map items into Collections (folders) and Requests.
    /// 3. Persist everything in one logical transaction (Rust-level: all inserts
    ///    run before returning; if any fails the already-inserted rows may
    ///    remain, but the collection-level insert is always first, so a partial
    ///    import is recoverable).
    ///
    /// Returns the ID of the top-level imported collection.
    pub async fn import_postman_v21(
        db: DbHandle,
        json: &str,
    ) -> Result<String, String> {
        // --- 1. Parse ---
        let collection: PostmanCollection = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse Postman JSON: {e}"))?;

        // --- 2. Validate version ---
        if !collection.info.schema.contains("v2.1") {
            return Err(format!(
                "Unsupported Postman schema version. Expected a schema URL containing 'v2.1', \
                 got '{}'.",
                collection.info.schema
            ));
        }

        let collection_name = sanitize_name(Some(&collection.info.name), "Imported Collection");
        let description = description_from_value(None); // top-level has no description field

        // --- 3. Create the root collection ---
        let root = collection_svc::create_collection(
            db.clone(),
            collection_name,
            description,
            None, // top-level
            crate::models::AuthConfig::None,
        )
        .await?;

        // --- 4. Recursively import items ---
        Self::import_items(db, &collection.item, root.id.clone(), 0).await?;

        Ok(root.id)
    }

    /// Recursively import a slice of `PostmanItem`s under `parent_collection_id`.
    ///
    /// - Folder items (those with `item`) become sub-collections.
    /// - Leaf items (those with `request`) become Requests.
    /// - `depth` is informational only (no limit enforced).
    async fn import_items(
        db: DbHandle,
        items: &[PostmanItem],
        parent_collection_id: String,
        depth: usize,
    ) -> Result<(), String> {
        for item in items {
            if let Some(sub_items) = &item.item {
                // --- Folder → sub-collection ---
                let folder_name = sanitize_name(item.name.as_deref(), "Unnamed Folder");
                let description = description_from_value(item.description.as_ref());

                let folder = collection_svc::create_collection(
                    db.clone(),
                    folder_name,
                    description,
                    Some(parent_collection_id.clone()),
                    crate::models::AuthConfig::None,
                )
                .await?;

                // Recurse into sub-items.
                Box::pin(Self::import_items(
                    db.clone(),
                    sub_items,
                    folder.id.clone(),
                    depth + 1,
                ))
                .await?;
            } else if let Some(request) = &item.request {
                // --- Leaf → request ---
                let name = sanitize_name(item.name.as_deref(), "Unnamed Request");
                let method = map_method(request.method.as_deref());
                let url = request
                    .url
                    .as_ref()
                    .map(|u| u.to_string_url())
                    .unwrap_or_default();
                let headers = map_headers(request.header.as_ref());
                let params = request
                    .url
                    .as_ref()
                    .map(|u| u.query_params())
                    .unwrap_or_default();
                let body = map_body(request.body.as_ref());
                let auth = map_auth(request.auth.as_ref());

                collection_svc::create_request(
                    db.clone(),
                    parent_collection_id.clone(),
                    name,
                    method,
                    url,
                    headers,
                    params,
                    body,
                    auth,
                )
                .await?;
            }
            // Items that are neither a folder nor a request are silently skipped.
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Lightman JSON format v2.0
    // -----------------------------------------------------------------------

    /// Import a Lightman JSON export (v2.0) string into the database.
    ///
    /// Steps:
    /// 1. Parse and validate the envelope (`lightman_version` must be "2.0").
    /// 2. Recursively import the collection tree (collections + requests).
    ///    Every node gets a new UUID; IDs are remapped so sub-collection and
    ///    saved-response foreign keys stay consistent.
    /// 3. Import saved responses using the remapped request IDs.
    /// 4. Import history entries (request snapshots preserved as-is, new IDs).
    /// 5. Import environments + variables (new IDs, is_active forced to false
    ///    so the user can explicitly choose which env to activate).
    ///
    /// Returns the ID of the newly created root collection.
    pub async fn import_lightman(
        db: DbHandle,
        json: &str,
    ) -> Result<String, String> {
        // --- 1. Parse ---
        let export: LightmanExport = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse Lightman JSON: {e}"))?;

        // --- 2. Validate version (accept "2.0") ---
        if export.lightman_version != LIGHTMAN_FORMAT_VERSION {
            return Err(format!(
                "Unsupported Lightman format version '{}'. Expected '{}'.",
                export.lightman_version, LIGHTMAN_FORMAT_VERSION
            ));
        }

        let now = Utc::now().to_rfc3339();

        // --- 3. Recursively import the collection tree.
        //        request_id_map: old_request_id → new_request_id (for saved responses).
        let mut request_id_map: HashMap<String, String> = HashMap::new();
        let new_root_id = Box::pin(Self::import_collection_node(
            db.clone(),
            &export.collection,
            None, // forced top-level
            &now,
            &mut request_id_map,
        ))
        .await?;

        // --- 4. Import saved responses (using remapped request IDs).
        for group in &export.collection_saved_responses() {
            if let Some(new_req_id) = request_id_map.get(&group.request_id) {
                for resp in &group.responses {
                    let new_id = Uuid::new_v4().to_string();
                    let headers_json = serde_json::to_string(&resp.headers)
                        .map_err(|e| format!("Failed to serialize saved response headers: {e}"))?;

                    crate::storage::insert_saved_response(
                        db.clone(),
                        new_id,
                        new_req_id.clone(),
                        resp.label.clone(),
                        resp.status_code as i64,
                        resp.status_text.clone(),
                        headers_json,
                        resp.body.clone(),
                        resp.response_time_ms as i64,
                        resp.response_size_bytes as i64,
                        resp.saved_at.clone(),
                    )
                    .await?;
                }
            }
        }

        // --- 5. Import history entries (new IDs, snapshot preserved).
        for entry in &export.history {
            let new_entry_id = Uuid::new_v4().to_string();
            let request_json = serde_json::to_string(&entry.request)
                .map_err(|e| format!("Failed to serialize history request: {e}"))?;
            let response_headers_json = serde_json::to_string(&entry.response_headers)
                .map_err(|e| format!("Failed to serialize history response headers: {e}"))?;

            crate::storage::insert_history_entry(
                db.clone(),
                new_entry_id,
                request_json,
                entry.status_code as i64,
                entry.status_text.clone(),
                entry.response_time_ms as i64,
                entry.response_size_bytes as i64,
                response_headers_json,
                entry.response_body.clone(),
                entry.executed_at.clone(),
            )
            .await?;
        }

        // --- 6. Import environments + variables.
        for env_export in &export.environments {
            let env = &env_export.environment;
            let new_env_id = Uuid::new_v4().to_string();

            // Determine position (count existing environments for order).
            // All imported envs start as inactive; user activates manually.
            crate::storage::insert_environment(
                db.clone(),
                new_env_id.clone(),
                env.name.clone(),
                false, // never import as active
                now.clone(),
                now.clone(),
            )
            .await?;

            // Insert variables for this environment.
            for var in &env_export.variables {
                let new_var_id = Uuid::new_v4().to_string();
                crate::storage::insert_variable(
                    db.clone(),
                    new_var_id,
                    Some(new_env_id.clone()),
                    var.key.clone(),
                    var.value.clone(),
                    var.enabled,
                )
                .await?;
            }
        }

        Ok(new_root_id)
    }

    /// Recursively import a `CollectionNode` (collection + requests + sub-nodes).
    ///
    /// `parent_id` overrides the node's stored `parent_id` so we always place the
    /// root at the top level regardless of where it originated.
    ///
    /// Fills `request_id_map` with old → new request ID mappings.
    async fn import_collection_node(
        db: DbHandle,
        node: &CollectionNode,
        parent_id: Option<String>,
        now: &str,
        request_id_map: &mut HashMap<String, String>,
    ) -> Result<String, String> {
        let col = &node.collection;
        let new_col_id = Uuid::new_v4().to_string();

        // Determine position within the parent.
        let existing = crate::storage::list_collections(db.clone()).await?;
        let position = existing
            .iter()
            .filter(|r| r.parent_id == parent_id)
            .count() as i64;

        let auth_json = serde_json::to_string(&col.auth)
            .unwrap_or_else(|_| r#"{"type":"none"}"#.to_string());

        crate::storage::insert_collection(
            db.clone(),
            new_col_id.clone(),
            col.name.clone(),
            col.description.clone(),
            parent_id,
            position,
            auth_json,
            now.to_string(),
            now.to_string(),
        )
        .await?;

        // Import requests in position order.
        let mut sorted_requests = node.requests.clone();
        sorted_requests.sort_by_key(|r| r.position);

        for (new_pos, req) in sorted_requests.iter().enumerate() {
            let new_req_id = Uuid::new_v4().to_string();
            request_id_map.insert(req.id.clone(), new_req_id.clone());

            let method_str = req.method.to_string();
            let headers_json = serde_json::to_string(&req.headers)
                .map_err(|e| format!("Failed to serialize request headers: {e}"))?;
            let params_json = serde_json::to_string(&req.params)
                .map_err(|e| format!("Failed to serialize request params: {e}"))?;
            let body_json = serde_json::to_string(&req.body)
                .map_err(|e| format!("Failed to serialize request body: {e}"))?;
            let auth_json = serde_json::to_string(&req.auth)
                .map_err(|e| format!("Failed to serialize request auth: {e}"))?;

            crate::storage::insert_request(
                db.clone(),
                new_req_id,
                new_col_id.clone(),
                req.name.clone(),
                method_str,
                req.url.clone(),
                headers_json,
                params_json,
                body_json,
                auth_json,
                new_pos as i64,
                now.to_string(),
                now.to_string(),
            )
            .await?;
        }

        // Recursively import sub-collections.
        let mut sorted_subs = node.sub_collections.clone();
        sorted_subs.sort_by_key(|n| n.collection.position);

        for sub in &sorted_subs {
            Box::pin(Self::import_collection_node(
                db.clone(),
                sub,
                Some(new_col_id.clone()),
                now,
                request_id_map,
            ))
            .await?;
        }

        Ok(new_col_id)
    }
} // end impl ImportService

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a plain-text description from a serde_json::Value (string or object
/// with a `content` field).
fn description_from_value(value: Option<&serde_json::Value>) -> Option<String> {
    let v = value?;
    match v {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() { None } else { Some(s.to_string()) }
        }
        serde_json::Value::Object(obj) => {
            obj.get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::collection as collection_svc;
    use crate::services::export::ExportService;
    use crate::storage;

    fn test_db() -> (DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_import_{}.db", Uuid::new_v4()));
        let handle = storage::initialize_database(path.clone())
            .expect("initialize_database");
        (handle, path)
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    // -----------------------------------------------------------------------
    // Postman v2.1 tests
    // -----------------------------------------------------------------------

    fn minimal_postman_json(name: &str) -> String {
        serde_json::json!({
            "info": {
                "name": name,
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": []
        })
        .to_string()
    }

    #[test]
    fn import_postman_wrong_schema_version_fails() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Old",
                "schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"
            },
            "item": []
        })
        .to_string();

        rt().block_on(async {
            let result = ImportService::import_postman_v21(db, &json).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(msg.contains("v2.1"), "unexpected error: {msg}");
        });
    }

    #[test]
    fn import_postman_invalid_json_fails() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result = ImportService::import_postman_v21(db, "not json").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn import_postman_empty_collection_creates_collection() {
        let (db, _) = test_db();
        rt().block_on(async {
            let json = minimal_postman_json("Empty API");
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let col = collection_svc::get_collection(db.clone(), col_id.clone())
                .await
                .unwrap()
                .expect("collection should exist");

            assert_eq!(col.name, "Empty API");
            assert_eq!(col.parent_id, None);

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert!(requests.is_empty());
        });
    }

    #[test]
    fn import_postman_flat_requests_are_created() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Flat API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Get Users",
                    "request": {
                        "method": "GET",
                        "url": "https://api.example.com/users",
                        "header": []
                    }
                },
                {
                    "name": "Create User",
                    "request": {
                        "method": "POST",
                        "url": "https://api.example.com/users",
                        "header": [
                            { "key": "Content-Type", "value": "application/json" }
                        ],
                        "body": {
                            "mode": "raw",
                            "raw": "{\"name\":\"Alice\"}",
                            "options": { "raw": { "language": "json" } }
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert_eq!(requests.len(), 2);

            let get_req = requests.iter().find(|r| r.name == "Get Users").unwrap();
            assert_eq!(get_req.method, HttpMethod::Get);
            assert_eq!(get_req.url, "https://api.example.com/users");

            let post_req = requests.iter().find(|r| r.name == "Create User").unwrap();
            assert_eq!(post_req.method, HttpMethod::Post);
            assert_eq!(post_req.headers.len(), 1);
            assert_eq!(post_req.headers[0].key, "Content-Type");
            assert!(
                matches!(&post_req.body, RequestBody::Json { content } if content.contains("Alice"))
            );
        });
    }

    #[test]
    fn import_postman_folder_creates_sub_collection() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Nested API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Users",
                    "item": [
                        {
                            "name": "List Users",
                            "request": {
                                "method": "GET",
                                "url": "https://api.example.com/users"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let root_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            // Root collection should have no direct requests.
            let root_requests = collection_svc::list_requests(db.clone(), root_id.clone())
                .await
                .unwrap();
            assert!(root_requests.is_empty(), "root should have no direct requests");

            // Sub-collection "Users" should exist as a child of root.
            let all_cols = collection_svc::list_collections(db.clone()).await.unwrap();
            let folder = all_cols
                .iter()
                .find(|c| c.name == "Users")
                .expect("Users folder should exist");
            assert_eq!(folder.parent_id, Some(root_id.clone()));

            // "List Users" request should live inside the folder.
            let folder_requests = collection_svc::list_requests(db, folder.id.clone())
                .await
                .unwrap();
            assert_eq!(folder_requests.len(), 1);
            assert_eq!(folder_requests[0].name, "List Users");
        });
    }

    #[test]
    fn import_postman_basic_auth_is_mapped() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Auth API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Protected",
                    "request": {
                        "method": "GET",
                        "url": "https://api.example.com/secret",
                        "auth": {
                            "type": "basic",
                            "basic": [
                                { "key": "username", "value": "admin", "type": "string" },
                                { "key": "password", "value": "pass123", "type": "string" }
                            ]
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert_eq!(requests.len(), 1);

            let auth = &requests[0].auth;
            assert!(
                matches!(auth, AuthConfig::BasicAuth { username, password }
                    if username == "admin" && password == "pass123"),
                "unexpected auth: {auth:?}"
            );
        });
    }

    #[test]
    fn import_postman_bearer_auth_is_mapped() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Bearer API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Token endpoint",
                    "request": {
                        "method": "GET",
                        "url": "https://api.example.com/me",
                        "auth": {
                            "type": "bearer",
                            "bearer": [
                                { "key": "token", "value": "mytoken123", "type": "string" }
                            ]
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert!(
                matches!(&requests[0].auth, AuthConfig::BearerToken { token } if token == "mytoken123")
            );
        });
    }

    #[test]
    fn import_postman_apikey_auth_is_mapped() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "ApiKey API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "API Key Request",
                    "request": {
                        "method": "GET",
                        "url": "https://api.example.com/data",
                        "auth": {
                            "type": "apikey",
                            "apikey": [
                                { "key": "key", "value": "X-Api-Key", "type": "string" },
                                { "key": "value", "value": "mysecretkey", "type": "string" },
                                { "key": "in", "value": "header", "type": "string" }
                            ]
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert!(
                matches!(
                    &requests[0].auth,
                    AuthConfig::ApiKey { key, value, location }
                        if key == "X-Api-Key"
                            && value == "mysecretkey"
                            && *location == ApiKeyLocation::Header
                )
            );
        });
    }

    #[test]
    fn import_postman_urlencoded_body_is_mapped() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Form API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Login",
                    "request": {
                        "method": "POST",
                        "url": "https://api.example.com/login",
                        "body": {
                            "mode": "urlencoded",
                            "urlencoded": [
                                { "key": "username", "value": "alice" },
                                { "key": "password", "value": "secret", "disabled": false }
                            ]
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert!(
                matches!(
                    &requests[0].body,
                    RequestBody::FormUrlEncoded { fields } if fields.len() == 2
                )
            );
        });
    }

    #[test]
    fn import_postman_url_object_with_query_params() {
        let (db, _) = test_db();
        let json = serde_json::json!({
            "info": {
                "name": "Params API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Search",
                    "request": {
                        "method": "GET",
                        "url": {
                            "raw": "https://api.example.com/search?q=rust&limit=10",
                            "host": ["api", "example", "com"],
                            "path": ["search"],
                            "query": [
                                { "key": "q", "value": "rust" },
                                { "key": "limit", "value": "10" }
                            ]
                        }
                    }
                }
            ]
        })
        .to_string();

        rt().block_on(async {
            let col_id = ImportService::import_postman_v21(db.clone(), &json)
                .await
                .unwrap();

            let requests = collection_svc::list_requests(db, col_id).await.unwrap();
            assert_eq!(requests[0].url, "https://api.example.com/search?q=rust&limit=10");
            assert_eq!(requests[0].params.len(), 2);
            assert_eq!(requests[0].params[0].key, "q");
            assert_eq!(requests[0].params[0].value, "rust");
        });
    }

    // -----------------------------------------------------------------------
    // Lightman import tests (v2.0 format)
    // -----------------------------------------------------------------------

    #[test]
    fn import_lightman_wrong_version_fails() {
        let (db, _) = test_db();
        // Build a JSON blob that looks like the new envelope shape but with
        // an unsupported version string.
        let json = serde_json::json!({
            "lightman_version": "99.0",
            "exported_at": "2024-01-01T00:00:00Z",
            "collection": {
                "collection": {
                    "id": "col-1", "name": "Test", "description": null,
                    "parent_id": null, "position": 0,
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z",
                    "default_headers": [], "auth": {"type": "none"}
                },
                "requests": [],
                "sub_collections": [],
                "saved_responses": []
            },
            "history": [],
            "environments": []
        })
        .to_string();

        rt().block_on(async {
            let result = ImportService::import_lightman(db, &json).await;
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(msg.contains("Unsupported"), "unexpected error: {msg}");
        });
    }

    #[test]
    fn import_lightman_invalid_json_fails() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result = ImportService::import_lightman(db, "not json").await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn import_lightman_round_trip() {
        let (db, _) = test_db();
        rt().block_on(async {
            use crate::models::{AuthConfig, HttpMethod, RequestBody};

            // 1. Create a collection with a request.
            let col = collection_svc::create_collection(
                db.clone(),
                "My Service".to_string(),
                Some("desc".to_string()),
                None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            collection_svc::create_request(
                db.clone(),
                col.id.clone(),
                "List Items".to_string(),
                HttpMethod::Get,
                "https://api.example.com/items".to_string(),
                vec![Header {
                    key: "Accept".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                }],
                vec![],
                RequestBody::None,
                AuthConfig::BearerToken {
                    token: "tok_abc".to_string(),
                },
            )
            .await
            .unwrap();

            // 2. Export to JSON.
            let export = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .unwrap();
            let json = ExportService::to_json(&export).unwrap();

            // 3. Import the JSON on the same DB (a re-import scenario).
            let new_col_id = ImportService::import_lightman(db.clone(), &json)
                .await
                .unwrap();

            // 4. Verify the imported collection is distinct (new ID) but same content.
            assert_ne!(new_col_id, col.id, "imported collection must have a fresh ID");

            let new_col = collection_svc::get_collection(db.clone(), new_col_id.clone())
                .await
                .unwrap()
                .expect("imported collection should exist");

            assert_eq!(new_col.name, "My Service");
            assert_eq!(new_col.description, Some("desc".to_string()));

            let new_requests = collection_svc::list_requests(db, new_col_id).await.unwrap();
            assert_eq!(new_requests.len(), 1);

            let req = &new_requests[0];
            assert_eq!(req.name, "List Items");
            assert_eq!(req.method, HttpMethod::Get);
            assert_eq!(req.url, "https://api.example.com/items");
            assert_eq!(req.headers.len(), 1);
            assert_eq!(req.headers[0].key, "Accept");
            assert!(
                matches!(&req.auth, AuthConfig::BearerToken { token } if token == "tok_abc")
            );
        });
    }

    #[test]
    fn import_lightman_preserves_request_order() {
        let (db, _) = test_db();
        rt().block_on(async {
            use crate::models::{AuthConfig, HttpMethod, RequestBody};

            let col = collection_svc::create_collection(
                db.clone(),
                "Ordered".to_string(),
                None,
                None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            for i in 0..5_u8 {
                collection_svc::create_request(
                    db.clone(),
                    col.id.clone(),
                    format!("Request {i}"),
                    HttpMethod::Get,
                    format!("https://example.com/{i}"),
                    vec![],
                    vec![],
                    RequestBody::None,
                    AuthConfig::None,
                )
                .await
                .unwrap();
            }

            let export = ExportService::export_collection(db.clone(), col.id.clone())
                .await
                .unwrap();
            let json = ExportService::to_json(&export).unwrap();

            let new_col_id = ImportService::import_lightman(db.clone(), &json)
                .await
                .unwrap();

            let new_requests = collection_svc::list_requests(db, new_col_id).await.unwrap();
            assert_eq!(new_requests.len(), 5);

            // Positions should be contiguous 0-4.
            let positions: Vec<i64> = new_requests.iter().map(|r| r.position).collect();
            let mut sorted = positions.clone();
            sorted.sort();
            assert_eq!(positions, sorted);
            assert_eq!(sorted, vec![0, 1, 2, 3, 4]);

            // Name order should match the original.
            let names: Vec<&str> = new_requests.iter().map(|r| r.name.as_str()).collect();
            assert_eq!(names, ["Request 0", "Request 1", "Request 2", "Request 3", "Request 4"]);
        });
    }

    #[test]
    fn import_lightman_includes_sub_collections() {
        let (db, _) = test_db();
        rt().block_on(async {
            use crate::models::{AuthConfig, HttpMethod, RequestBody};

            let root = collection_svc::create_collection(
                db.clone(), "Root".to_string(), None, None, AuthConfig::None,
            )
            .await
            .unwrap();

            let folder = collection_svc::create_collection(
                db.clone(), "Folder".to_string(), None, Some(root.id.clone()), AuthConfig::None,
            )
            .await
            .unwrap();

            collection_svc::create_request(
                db.clone(), folder.id.clone(), "Sub Req".to_string(),
                HttpMethod::Get, "https://example.com/sub".to_string(),
                vec![], vec![], RequestBody::None, AuthConfig::None,
            )
            .await
            .unwrap();

            let export = ExportService::export_collection(db.clone(), root.id.clone())
                .await
                .unwrap();
            let json = ExportService::to_json(&export).unwrap();

            let new_root_id = ImportService::import_lightman(db.clone(), &json)
                .await
                .unwrap();

            // Verify sub-collection was imported.
            let all_cols = collection_svc::list_collections(db.clone()).await.unwrap();
            let new_folder = all_cols
                .iter()
                .find(|c| c.name == "Folder" && c.parent_id.as_deref() == Some(&new_root_id))
                .expect("imported folder should exist as child of new root");

            let sub_reqs = collection_svc::list_requests(db, new_folder.id.clone()).await.unwrap();
            assert_eq!(sub_reqs.len(), 1);
            assert_eq!(sub_reqs[0].name, "Sub Req");
        });
    }
} // end mod tests
