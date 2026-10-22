// Tauri IPC commands for Collection, Folder, and Request management.
// These are the entry points called by the React frontend via invoke().
//
// Task 7.2: Expose all CollectionService functions as #[tauri::command] async functions.
//
// Requirements: 2.1, 2.2, 8.3

use crate::models::{AuthConfig, Collection, Header, HttpMethod, QueryParam, Request, RequestBody};
use crate::services::collection as svc;
use crate::storage::DbHandle;
use tauri::State;

// ---------------------------------------------------------------------------
// Collection commands
// ---------------------------------------------------------------------------

/// Create a new collection (or nested folder when `parent_id` is provided).
///
/// # Frontend call
/// ```ts
/// invoke<Collection>('create_collection', { name, description, parentId, auth })
/// ```
#[tauri::command]
pub async fn create_collection(
    db: State<'_, DbHandle>,
    name: String,
    description: Option<String>,
    parent_id: Option<String>,
    auth: Option<AuthConfig>,
) -> Result<Collection, String> {
    svc::create_collection(
        db.inner().clone(),
        name,
        description,
        parent_id,
        auth.unwrap_or(AuthConfig::None),
    )
    .await
}

/// Fetch a single collection by ID.
///
/// Returns `null` on the JS side when no collection with that ID exists.
///
/// # Frontend call
/// ```ts
/// invoke<Collection | null>('get_collection', { id })
/// ```
#[tauri::command]
pub async fn get_collection(
    db: State<'_, DbHandle>,
    id: String,
) -> Result<Option<Collection>, String> {
    svc::get_collection(db.inner().clone(), id).await
}

/// Return all collections (top-level and nested), ordered by parent then position.
///
/// # Frontend call
/// ```ts
/// invoke<Collection[]>('list_collections')
/// ```
#[tauri::command]
pub async fn list_collections(db: State<'_, DbHandle>) -> Result<Vec<Collection>, String> {
    svc::list_collections(db.inner().clone()).await
}

/// Update the name, description, and/or auth of an existing collection.
///
/// # Frontend call
/// ```ts
/// invoke<Collection>('update_collection', { id, name, description, auth })
/// ```
#[tauri::command]
pub async fn update_collection(
    db: State<'_, DbHandle>,
    id: String,
    name: String,
    description: Option<String>,
    auth: Option<AuthConfig>,
) -> Result<Collection, String> {
    svc::update_collection(
        db.inner().clone(),
        id,
        name,
        description,
        auth.unwrap_or(AuthConfig::None),
    )
    .await
}

/// Delete a collection (and all its descendants) by ID.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_collection', { id })
/// ```
#[tauri::command]
pub async fn delete_collection(db: State<'_, DbHandle>, id: String) -> Result<(), String> {
    svc::delete_collection(db.inner().clone(), id).await
}

// ---------------------------------------------------------------------------
// Request commands
// ---------------------------------------------------------------------------

/// Create a new HTTP request inside a collection.
///
/// # Frontend call
/// ```ts
/// invoke<Request>('create_request', { collectionId, name, method, url, headers, params, body, auth })
/// ```
#[tauri::command]
pub async fn create_request(
    db: State<'_, DbHandle>,
    collection_id: String,
    name: String,
    method: HttpMethod,
    url: String,
    headers: Vec<Header>,
    params: Vec<QueryParam>,
    body: RequestBody,
    auth: AuthConfig,
) -> Result<Request, String> {
    svc::create_request(
        db.inner().clone(),
        collection_id,
        name,
        method,
        url,
        headers,
        params,
        body,
        auth,
    )
    .await
}

/// Fetch a single request by ID.
///
/// Returns `null` on the JS side when no request with that ID exists.
///
/// # Frontend call
/// ```ts
/// invoke<Request | null>('get_request', { id })
/// ```
#[tauri::command]
pub async fn get_request(
    db: State<'_, DbHandle>,
    id: String,
) -> Result<Option<Request>, String> {
    svc::get_request(db.inner().clone(), id).await
}

/// Return all requests belonging to a collection, ordered by position.
///
/// # Frontend call
/// ```ts
/// invoke<Request[]>('list_requests', { collectionId })
/// ```
#[tauri::command]
pub async fn list_requests(
    db: State<'_, DbHandle>,
    collection_id: String,
) -> Result<Vec<Request>, String> {
    svc::list_requests(db.inner().clone(), collection_id).await
}

/// Update all mutable fields of an existing request.
///
/// # Frontend call
/// ```ts
/// invoke<Request>('update_request', { id, name, method, url, headers, params, body, auth })
/// ```
#[tauri::command]
pub async fn update_request(
    db: State<'_, DbHandle>,
    id: String,
    name: String,
    method: HttpMethod,
    url: String,
    headers: Vec<Header>,
    params: Vec<QueryParam>,
    body: RequestBody,
    auth: AuthConfig,
) -> Result<Request, String> {
    svc::update_request(
        db.inner().clone(),
        id,
        name,
        method,
        url,
        headers,
        params,
        body,
        auth,
    )
    .await
}

/// Delete a request by ID.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_request', { id })
/// ```
#[tauri::command]
pub async fn delete_request(db: State<'_, DbHandle>, id: String) -> Result<(), String> {
    svc::delete_request(db.inner().clone(), id).await
}

/// Duplicate an existing request within the same collection.
///
/// The copy receives a new UUID, the name suffix " (Copy)", and is placed
/// at the end of the collection.
///
/// # Frontend call
/// ```ts
/// invoke<Request>('duplicate_request', { id })
/// ```
#[tauri::command]
pub async fn duplicate_request(db: State<'_, DbHandle>, id: String) -> Result<Request, String> {
    svc::duplicate_request(db.inner().clone(), id).await
}

/// Move a request to a different collection (or folder).
///
/// The request is appended at the end of the target collection.
///
/// # Frontend call
/// ```ts
/// invoke<Request>('move_request', { id, targetCollectionId })
/// ```
#[tauri::command]
pub async fn move_request(
    db: State<'_, DbHandle>,
    id: String,
    target_collection_id: String,
) -> Result<Request, String> {
    svc::move_request(db.inner().clone(), id, target_collection_id).await
}

/// Move a request to a new zero-based position within its collection.
///
/// All sibling requests are shifted automatically to keep positions
/// contiguous. `target_position` is clamped to the valid range.
///
/// # Frontend call
/// ```ts
/// invoke<void>('reorder_request', { id, targetPosition })
/// ```
#[tauri::command]
pub async fn reorder_request(
    db: State<'_, DbHandle>,
    id: String,
    target_position: i64,
) -> Result<(), String> {
    svc::reorder_request(db.inner().clone(), id, target_position).await
}
