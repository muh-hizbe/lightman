// Collection and Request service — business logic layer between commands and storage.
// Task 7.1: CRUD operations for Collections and Requests with validation.
//
// Requirements: 2.1, 2.2, 2.4, 2.6, 2.7

use crate::models::{AuthConfig, Collection, Header, Request};
use crate::storage::{
    self, CollectionRow, DbHandle, RequestRow,
};
use chrono::Utc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate that a `name` is non-empty and at most 255 characters.
/// Returns `Err` with a descriptive message on violation.
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Name must not be empty.".to_string());
    }
    if name.len() > 255 {
        return Err("Name must not exceed 255 characters.".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Row → Domain model converters
// ---------------------------------------------------------------------------

/// Convert a `CollectionRow` from storage into a `Collection` domain model.
fn collection_from_row(row: CollectionRow) -> Result<Collection, String> {
    let auth = serde_json::from_str(&row.auth_json)
        .map_err(|e| format!("Failed to parse collection auth_json: {e}"))?;

    Ok(Collection {
        id: row.id,
        name: row.name,
        description: row.description,
        parent_id: row.parent_id,
        position: row.position,
        created_at: row.created_at,
        updated_at: row.updated_at,
        default_headers: vec![],
        auth,
    })
}

/// Convert a `RequestRow` from storage into a `Request` domain model by
/// deserialising the JSON columns.
fn request_from_row(row: RequestRow) -> Result<Request, String> {
    let method = serde_json::from_str(&format!("\"{}\"", row.method))
        .map_err(|e| format!("Failed to parse method '{}': {e}", row.method))?;

    let headers = serde_json::from_str(&row.headers_json)
        .map_err(|e| format!("Failed to parse headers_json: {e}"))?;

    let params = serde_json::from_str(&row.params_json)
        .map_err(|e| format!("Failed to parse params_json: {e}"))?;

    let body = serde_json::from_str(&row.body_json)
        .map_err(|e| format!("Failed to parse body_json: {e}"))?;

    let auth = serde_json::from_str(&row.auth_json)
        .map_err(|e| format!("Failed to parse auth_json: {e}"))?;

    Ok(Request {
        id: row.id,
        collection_id: row.collection_id,
        name: row.name,
        method,
        url: row.url,
        headers,
        params,
        body,
        auth,
        position: row.position,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

// ---------------------------------------------------------------------------
// Collection operations (Requirement 2.1, 2.2)
// ---------------------------------------------------------------------------

/// Create a new top-level collection or nested folder.
///
/// # Arguments
/// * `name` — Display name, 1–255 characters.
/// * `description` — Optional free-text description.
/// * `parent_id` — Parent collection ID; `None` for a top-level collection.
/// * `auth` — Authentication configuration for this collection/folder.
///
/// Returns the newly created `Collection`.
pub async fn create_collection(
    db: DbHandle,
    name: String,
    description: Option<String>,
    parent_id: Option<String>,
    auth: AuthConfig,
) -> Result<Collection, String> {
    validate_name(&name)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Determine the next position: count existing siblings and place at the end.
    let siblings = storage::list_collections(db.clone()).await?;
    let position = siblings
        .iter()
        .filter(|r| r.parent_id == parent_id)
        .count() as i64;

    let auth_json = serde_json::to_string(&auth)
        .map_err(|e| format!("Failed to serialise collection auth: {e}"))?;

    storage::insert_collection(
        db.clone(),
        id.clone(),
        name.clone(),
        description.clone(),
        parent_id.clone(),
        position,
        auth_json,
        now.clone(),
        now.clone(),
    )
    .await?;

    let row = storage::get_collection(db, id)
        .await?
        .ok_or_else(|| "Collection was not found immediately after insert.".to_string())?;

    collection_from_row(row)
}

/// Fetch a single collection by its ID.
/// Returns `None` if no collection with that ID exists.
pub async fn get_collection(
    db: DbHandle,
    id: String,
) -> Result<Option<Collection>, String> {
    match storage::get_collection(db, id).await? {
        Some(row) => Ok(Some(collection_from_row(row)?)),
        None => Ok(None),
    }
}

/// Return all collections (top-level and nested) ordered by parent then position.
pub async fn list_collections(db: DbHandle) -> Result<Vec<Collection>, String> {
    let rows = storage::list_collections(db).await?;
    rows.into_iter().map(collection_from_row).collect()
}

/// Update the name, description, and/or auth of an existing collection.
///
/// Returns `Err` if no collection with `id` exists or if the new name is invalid.
pub async fn update_collection(
    db: DbHandle,
    id: String,
    name: String,
    description: Option<String>,
    auth: AuthConfig,
) -> Result<Collection, String> {
    validate_name(&name)?;

    // Verify the collection exists before attempting the update.
    storage::get_collection(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Collection '{id}' not found."))?;

    let now = Utc::now().to_rfc3339();
    let auth_json = serde_json::to_string(&auth)
        .map_err(|e| format!("Failed to serialise collection auth: {e}"))?;

    storage::update_collection(db.clone(), id.clone(), name, description, auth_json, now).await?;

    let row = storage::get_collection(db, id.clone())
        .await?
        .ok_or_else(|| format!("Collection '{id}' not found after update."))?;

    collection_from_row(row)
}

/// Delete a collection and all its descendants (sub-folders and requests)
/// via SQLite's `ON DELETE CASCADE` foreign key constraint.
pub async fn delete_collection(db: DbHandle, id: String) -> Result<(), String> {
    // Verify the collection exists.
    storage::get_collection(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Collection '{id}' not found."))?;

    storage::delete_collection(db, id).await
}

// ---------------------------------------------------------------------------
// Request operations (Requirements 2.2, 2.4, 2.6, 2.7)
// ---------------------------------------------------------------------------

/// Create a new HTTP request inside a collection.
pub async fn create_request(
    db: DbHandle,
    collection_id: String,
    name: String,
    method: crate::models::HttpMethod,
    url: String,
    headers: Vec<Header>,
    params: Vec<crate::models::QueryParam>,
    body: crate::models::RequestBody,
    auth: crate::models::AuthConfig,
) -> Result<Request, String> {
    validate_name(&name)?;

    // Verify the parent collection exists.
    storage::get_collection(db.clone(), collection_id.clone())
        .await?
        .ok_or_else(|| format!("Collection '{collection_id}' not found."))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Position = number of existing requests in this collection.
    let siblings = storage::list_requests_by_collection(db.clone(), collection_id.clone()).await?;
    let position = siblings.len() as i64;

    let method_str = method.to_string();
    let headers_json = serde_json::to_string(&headers)
        .map_err(|e| format!("Failed to serialise headers: {e}"))?;
    let params_json = serde_json::to_string(&params)
        .map_err(|e| format!("Failed to serialise params: {e}"))?;
    let body_json = serde_json::to_string(&body)
        .map_err(|e| format!("Failed to serialise body: {e}"))?;
    let auth_json = serde_json::to_string(&auth)
        .map_err(|e| format!("Failed to serialise auth: {e}"))?;

    storage::insert_request(
        db.clone(),
        id.clone(),
        collection_id,
        name,
        method_str,
        url,
        headers_json,
        params_json,
        body_json,
        auth_json,
        position,
        now.clone(),
        now,
    )
    .await?;

    let row = storage::get_request(db, id)
        .await?
        .ok_or_else(|| "Request was not found immediately after insert.".to_string())?;

    request_from_row(row)
}

/// Fetch a single request by its ID.
/// Returns `None` if no request with that ID exists.
pub async fn get_request(db: DbHandle, id: String) -> Result<Option<Request>, String> {
    match storage::get_request(db, id).await? {
        Some(row) => Ok(Some(request_from_row(row)?)),
        None => Ok(None),
    }
}

/// Return all requests belonging to a collection, ordered by position.
pub async fn list_requests(
    db: DbHandle,
    collection_id: String,
) -> Result<Vec<Request>, String> {
    let rows = storage::list_requests_by_collection(db, collection_id).await?;
    rows.into_iter().map(request_from_row).collect()
}

/// Update all mutable fields of an existing request.
///
/// Returns `Err` if no request with `id` exists or if the new name is invalid.
pub async fn update_request(
    db: DbHandle,
    id: String,
    name: String,
    method: crate::models::HttpMethod,
    url: String,
    headers: Vec<Header>,
    params: Vec<crate::models::QueryParam>,
    body: crate::models::RequestBody,
    auth: crate::models::AuthConfig,
) -> Result<Request, String> {
    validate_name(&name)?;

    let existing_row = storage::get_request(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found."))?;

    let now = Utc::now().to_rfc3339();
    let method_str = method.to_string();
    let headers_json = serde_json::to_string(&headers)
        .map_err(|e| format!("Failed to serialise headers: {e}"))?;
    let params_json = serde_json::to_string(&params)
        .map_err(|e| format!("Failed to serialise params: {e}"))?;
    let body_json = serde_json::to_string(&body)
        .map_err(|e| format!("Failed to serialise body: {e}"))?;
    let auth_json = serde_json::to_string(&auth)
        .map_err(|e| format!("Failed to serialise auth: {e}"))?;

    storage::update_request(
        db.clone(),
        id.clone(),
        name,
        method_str,
        url,
        headers_json,
        params_json,
        body_json,
        auth_json,
        existing_row.position,
        now,
    )
    .await?;

    let row = storage::get_request(db, id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found after update."))?;

    request_from_row(row)
}

/// Delete a request by its ID.
pub async fn delete_request(db: DbHandle, id: String) -> Result<(), String> {
    storage::get_request(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found."))?;

    storage::delete_request(db, id).await
}

/// Move a request to a different collection/folder.
///
/// The request is placed at the end of the target collection.
/// Returns the updated `Request` with its new `collection_id`.
pub async fn move_request(
    db: DbHandle,
    id: String,
    target_collection_id: String,
) -> Result<Request, String> {
    storage::get_request(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found."))?;

    storage::get_collection(db.clone(), target_collection_id.clone())
        .await?
        .ok_or_else(|| format!("Target collection '{target_collection_id}' not found."))?;

    let now = Utc::now().to_rfc3339();
    storage::move_request(db.clone(), id.clone(), target_collection_id, now).await?;

    let row = storage::get_request(db, id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found after move."))?;

    request_from_row(row)
}

/// Duplicate an existing request within the same collection.
///
/// The duplicate gets a new UUID, the suffix " (Copy)" appended to the name,
/// and is placed at the end of the collection.
pub async fn duplicate_request(db: DbHandle, id: String) -> Result<Request, String> {
    let existing = storage::get_request(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found."))?;

    let new_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let suffix = " (Copy)";
    let copy_name = if existing.name.len() + suffix.len() <= 255 {
        format!("{}{suffix}", existing.name)
    } else {
        existing.name[..255 - suffix.len()].to_string() + suffix
    };

    let siblings = storage::list_requests_by_collection(db.clone(), existing.collection_id.clone()).await?;
    let position = siblings.len() as i64;

    storage::insert_request(
        db.clone(),
        new_id.clone(),
        existing.collection_id.clone(),
        copy_name,
        existing.method.clone(),
        existing.url.clone(),
        existing.headers_json.clone(),
        existing.params_json.clone(),
        existing.body_json.clone(),
        existing.auth_json.clone(),
        position,
        now.clone(),
        now,
    )
    .await?;

    let row = storage::get_request(db, new_id.clone())
        .await?
        .ok_or_else(|| format!("Duplicate request '{new_id}' not found after insert."))?;

    request_from_row(row)
}

/// Reorder a request within its collection.
///
/// `target_position` is clamped to `[0, count-1]`.
pub async fn reorder_request(
    db: DbHandle,
    id: String,
    target_position: i64,
) -> Result<(), String> {
    let req = storage::get_request(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Request '{id}' not found."))?;

    let siblings = storage::list_requests_by_collection(db.clone(), req.collection_id.clone()).await?;
    let count = siblings.len() as i64;
    let clamped = target_position.max(0).min(count.saturating_sub(1));

    if req.position == clamped {
        return Ok(());
    }

    // Remove from current position and re-insert at target.
    let mut ordered: Vec<_> = siblings.iter()
        .filter(|r| r.id != id)
        .collect();

    let insert_at = clamped as usize;
    let insert_at = insert_at.min(ordered.len());

    let req_clone = siblings.iter().find(|r| r.id == id).unwrap().clone();
    ordered.insert(insert_at, &req_clone);

    let now = Utc::now().to_rfc3339();
    for (new_pos, sibling) in ordered.iter().enumerate() {
        storage::update_request(
            db.clone(),
            sibling.id.clone(),
            sibling.name.clone(),
            sibling.method.clone(),
            sibling.url.clone(),
            sibling.headers_json.clone(),
            sibling.params_json.clone(),
            sibling.body_json.clone(),
            sibling.auth_json.clone(),
            new_pos as i64,
            now.clone(),
        )
        .await?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, RequestBody};
    use crate::storage;

    /// Open a file-backed SQLite database for this test.
    /// Returns `(DbHandle, PathBuf)`.  Drop the handle and remove the file to
    /// clean up (the OS also cleans temp files on process exit).
    fn test_db() -> (DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_svc_coll_{}.db", Uuid::new_v4()));
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

    // ---- validate_name ----

    #[test]
    fn validate_name_empty_is_error() {
        assert!(validate_name("").is_err());
    }

    #[test]
    fn validate_name_255_chars_is_ok() {
        let name = "a".repeat(255);
        assert!(validate_name(&name).is_ok());
    }

    #[test]
    fn validate_name_256_chars_is_error() {
        let name = "a".repeat(256);
        assert!(validate_name(&name).is_err());
    }

    // ---- Collection CRUD ----

    #[test]
    fn create_and_get_collection() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(
                db.clone(),
                "My API".to_string(),
                Some("desc".to_string()),
                None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            assert!(!col.id.is_empty());
            assert_eq!(col.name, "My API");
            assert_eq!(col.description, Some("desc".to_string()));
            assert_eq!(col.parent_id, None);
            assert_eq!(col.position, 0);

            let fetched = get_collection(db, col.id.clone())
                .await
                .unwrap()
                .expect("collection should exist");
            assert_eq!(fetched.id, col.id);
            assert_eq!(fetched.name, col.name);
        });
    }

    #[test]
    fn create_collection_empty_name_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_collection(db, "".to_string(), None, None, AuthConfig::None).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn create_collection_too_long_name_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_collection(db, "a".repeat(256), None, None, AuthConfig::None).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn list_collections_returns_all() {
        let (db, _path) = test_db();
        rt().block_on(async {
            create_collection(db.clone(), "Alpha".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();
            create_collection(db.clone(), "Beta".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let cols = list_collections(db).await.unwrap();
            assert_eq!(cols.len(), 2);
        });
    }

    #[test]
    fn update_collection_name() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Old Name".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let updated = update_collection(
                db.clone(),
                col.id.clone(),
                "New Name".to_string(),
                None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            assert_eq!(updated.name, "New Name");
            assert_eq!(updated.id, col.id);
        });
    }

    #[test]
    fn update_collection_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = update_collection(
                db,
                "nonexistent-id".to_string(),
                "Name".to_string(),
                None,
                AuthConfig::None,
            )
            .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn delete_collection_removes_it() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Temp".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            delete_collection(db.clone(), col.id.clone())
                .await
                .unwrap();

            let fetched = get_collection(db, col.id).await.unwrap();
            assert!(fetched.is_none());
        });
    }

    #[test]
    fn delete_collection_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = delete_collection(db, "nonexistent".to_string()).await;
            assert!(result.is_err());
        });
    }

    // ---- Collection position auto-assignment ----

    #[test]
    fn collections_position_auto_increments() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let c1 = create_collection(db.clone(), "First".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();
            let c2 = create_collection(db.clone(), "Second".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();
            let c3 = create_collection(db.clone(), "Third".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            assert_eq!(c1.position, 0);
            assert_eq!(c2.position, 1);
            assert_eq!(c3.position, 2);
        });
    }

    // ---- Request CRUD ----

    fn default_request_args() -> (
        String,
        HttpMethod,
        String,
        Vec<Header>,
        Vec<crate::models::QueryParam>,
        RequestBody,
        AuthConfig,
    ) {
        (
            "My Request".to_string(),
            HttpMethod::Get,
            "https://example.com".to_string(),
            vec![],
            vec![],
            RequestBody::None,
            AuthConfig::None,
        )
    }

    #[test]
    fn create_and_get_request() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let (name, method, url, headers, params, body, auth) = default_request_args();
            let req = create_request(
                db.clone(),
                col.id.clone(),
                name,
                method,
                url,
                headers,
                params,
                body,
                auth,
            )
            .await
            .unwrap();

            assert!(!req.id.is_empty());
            assert_eq!(req.collection_id, col.id);
            assert_eq!(req.name, "My Request");
            assert_eq!(req.method, HttpMethod::Get);
            assert_eq!(req.position, 0);

            let fetched = get_request(db, req.id.clone())
                .await
                .unwrap()
                .expect("request should exist");
            assert_eq!(fetched.id, req.id);
        });
    }

    #[test]
    fn create_request_empty_name_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let result = create_request(
                db,
                col.id,
                "".to_string(),
                HttpMethod::Get,
                "https://example.com".to_string(),
                vec![],
                vec![],
                RequestBody::None,
                AuthConfig::None,
            )
            .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn create_request_nonexistent_collection_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_request(
                db,
                "no-such-collection".to_string(),
                "Name".to_string(),
                HttpMethod::Get,
                "https://example.com".to_string(),
                vec![],
                vec![],
                RequestBody::None,
                AuthConfig::None,
            )
            .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn list_requests_returns_collection_requests() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            for i in 0..3 {
                create_request(
                    db.clone(),
                    col.id.clone(),
                    format!("Request {i}"),
                    HttpMethod::Get,
                    "https://example.com".to_string(),
                    vec![],
                    vec![],
                    RequestBody::None,
                    AuthConfig::None,
                )
                .await
                .unwrap();
            }

            let requests = list_requests(db, col.id).await.unwrap();
            assert_eq!(requests.len(), 3);
        });
    }

    #[test]
    fn update_request_changes_fields() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let (name, method, url, headers, params, body, auth) = default_request_args();
            let req = create_request(
                db.clone(),
                col.id.clone(),
                name,
                method,
                url,
                headers,
                params,
                body,
                auth,
            )
            .await
            .unwrap();

            let updated = update_request(
                db.clone(),
                req.id.clone(),
                "Updated".to_string(),
                HttpMethod::Post,
                "https://updated.example.com".to_string(),
                vec![],
                vec![],
                RequestBody::Json {
                    content: "{}".to_string(),
                },
                AuthConfig::None,
            )
            .await
            .unwrap();

            assert_eq!(updated.name, "Updated");
            assert_eq!(updated.method, HttpMethod::Post);
            assert_eq!(updated.url, "https://updated.example.com");
            // Position should be preserved.
            assert_eq!(updated.position, req.position);
        });
    }

    #[test]
    fn delete_request_removes_it() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let (name, method, url, headers, params, body, auth) = default_request_args();
            let req = create_request(
                db.clone(),
                col.id,
                name,
                method,
                url,
                headers,
                params,
                body,
                auth,
            )
            .await
            .unwrap();

            delete_request(db.clone(), req.id.clone()).await.unwrap();

            let fetched = get_request(db, req.id).await.unwrap();
            assert!(fetched.is_none());
        });
    }

    // ---- Duplicate request ----

    #[test]
    fn duplicate_request_creates_copy() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let req = create_request(
                db.clone(),
                col.id.clone(),
                "Original".to_string(),
                HttpMethod::Post,
                "https://example.com/api".to_string(),
                vec![],
                vec![],
                RequestBody::Json {
                    content: "{}".to_string(),
                },
                AuthConfig::None,
            )
            .await
            .unwrap();

            let copy = duplicate_request(db.clone(), req.id.clone())
                .await
                .unwrap();

            // Different ID, same collection.
            assert_ne!(copy.id, req.id);
            assert_eq!(copy.collection_id, col.id);
            assert_eq!(copy.name, "Original (Copy)");
            assert_eq!(copy.method, HttpMethod::Post);
            assert_eq!(copy.url, req.url);
            // Copy is placed after the original.
            assert!(copy.position > req.position);
        });
    }

    #[test]
    fn duplicate_request_long_name_truncated() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let long_name = "a".repeat(254); // 254 + " (Copy)" = 261 → must truncate
            let req = create_request(
                db.clone(),
                col.id.clone(),
                long_name,
                HttpMethod::Get,
                "https://example.com".to_string(),
                vec![],
                vec![],
                RequestBody::None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            let copy = duplicate_request(db.clone(), req.id).await.unwrap();
            assert!(copy.name.len() <= 255);
            assert!(copy.name.ends_with(" (Copy)"));
        });
    }

    // ---- Reorder request ----

    #[test]
    fn reorder_request_moves_item() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let mut ids = Vec::new();
            for i in 0..4 {
                let req = create_request(
                    db.clone(),
                    col.id.clone(),
                    format!("Req {i}"),
                    HttpMethod::Get,
                    "https://example.com".to_string(),
                    vec![],
                    vec![],
                    RequestBody::None,
                    AuthConfig::None,
                )
                .await
                .unwrap();
                ids.push(req.id);
            }

            // Move item at position 0 to position 3 (last).
            reorder_request(db.clone(), ids[0].clone(), 3).await.unwrap();

            let requests = list_requests(db, col.id).await.unwrap();
            // Positions should be 0-indexed and contiguous.
            let positions: Vec<i64> = requests.iter().map(|r| r.position).collect();
            assert_eq!(positions, vec![0, 1, 2, 3]);

            // The item that was originally at position 0 should now be at position 3.
            let moved = requests.iter().find(|r| r.id == ids[0]).unwrap();
            assert_eq!(moved.position, 3);
        });
    }

    #[test]
    fn reorder_request_clamps_out_of_bounds() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let col = create_collection(db.clone(), "Col".to_string(), None, None, AuthConfig::None)
                .await
                .unwrap();

            let req = create_request(
                db.clone(),
                col.id.clone(),
                "Only".to_string(),
                HttpMethod::Get,
                "https://example.com".to_string(),
                vec![],
                vec![],
                RequestBody::None,
                AuthConfig::None,
            )
            .await
            .unwrap();

            // Target position way out of range — should clamp to 0 (the only valid slot).
            reorder_request(db.clone(), req.id.clone(), 999)
                .await
                .unwrap();

            let fetched = get_request(db, req.id).await.unwrap().unwrap();
            assert_eq!(fetched.position, 0);
        });
    }
}
