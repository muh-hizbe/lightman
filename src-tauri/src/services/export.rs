// Export service — serializes a Collection tree (sub-collections, requests,
// saved responses, history entries) and all Environments with their variables
// to the Lightman JSON format v2.0.
//
// Requirements: 6.3, 6.4

use crate::models::{Collection, Environment, HistoryEntry, Request, SavedResponse, Variable};
use crate::storage::DbHandle;
use crate::services::collection as collection_svc;
use crate::services::environment as env_svc;
use crate::services::history as history_svc;
use crate::services::saved_response as saved_response_svc;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Current version of the Lightman export format.
pub const LIGHTMAN_FORMAT_VERSION: &str = "2.0";

// ---------------------------------------------------------------------------
// Export envelope
// ---------------------------------------------------------------------------

/// A collection node in the export tree, including its requests, sub-folders,
/// and saved responses keyed by the original request ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionNode {
    /// Collection metadata.
    pub collection: Collection,
    /// Requests directly inside this collection/folder, ordered by position.
    pub requests: Vec<Request>,
    /// Sub-collections/folders under this collection, ordered by position.
    pub sub_collections: Vec<CollectionNode>,
    /// Saved responses grouped by the original request ID.
    /// Key: original `request_id`; Value: list of saved responses for that request.
    pub saved_responses: Vec<SavedResponseGroup>,
}

/// Saved responses associated with a single request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedResponseGroup {
    /// Original request ID (from the exported collection).
    pub request_id: String,
    /// All saved response snapshots for that request.
    pub responses: Vec<SavedResponse>,
}

/// An environment bundled with all its variables for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentExport {
    pub environment: Environment,
    pub variables: Vec<Variable>,
}

/// The top-level JSON structure produced by an export operation.
///
/// Schema:
/// ```json
/// {
///   "lightman_version": "2.0",
///   "exported_at": "<ISO-8601>",
///   "collection": { … CollectionNode … },
///   "history": [ … HistoryEntry … ],
///   "environments": [ … EnvironmentExport … ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightmanExport {
    /// Format/schema version so importers can detect compatibility.
    pub lightman_version: String,
    /// ISO-8601 timestamp of when the export was created.
    pub exported_at: String,
    /// The exported collection tree (recursive).
    pub collection: CollectionNode,
    /// History entries whose request was part of this collection (matched by
    /// collection_id at the time of execution).
    pub history: Vec<HistoryEntry>,
    /// All environments and their variables.
    pub environments: Vec<EnvironmentExport>,
}

// ---------------------------------------------------------------------------
// LightmanExport helpers
// ---------------------------------------------------------------------------

impl LightmanExport {
    /// Flatten all `SavedResponseGroup`s from the entire collection tree.
    /// Used by the importer to iterate saved responses without recursing manually.
    pub fn collection_saved_responses(&self) -> Vec<crate::services::export::SavedResponseGroup> {
        let mut out = Vec::new();
        Self::collect_saved_responses(&self.collection, &mut out);
        out
    }

    fn collect_saved_responses(
        node: &CollectionNode,
        out: &mut Vec<SavedResponseGroup>,
    ) {
        for group in &node.saved_responses {
            out.push(group.clone());
        }
        for sub in &node.sub_collections {
            Self::collect_saved_responses(sub, out);
        }
    }
}

// ---------------------------------------------------------------------------
// ExportService
// ---------------------------------------------------------------------------

pub struct ExportService;

impl ExportService {
    /// Serialize a root-level `Collection` tree into a [`LightmanExport`].
    ///
    /// Only root-level collections (those with `parent_id == None`) may be
    /// exported.  Attempting to export a sub-collection (folder) returns an
    /// error immediately.
    ///
    /// The export includes:
    /// - The full collection tree (sub-collections + requests at every level)
    /// - Saved responses for every request in the tree
    /// - History entries whose embedded request had a `collection_id` anywhere
    ///   in this collection's tree
    /// - All environments and their variables
    ///
    /// # Errors
    /// Returns `Err` if the collection does not exist, is a folder, or any
    /// storage operation fails.
    pub async fn export_collection(
        db: DbHandle,
        collection_id: String,
    ) -> Result<LightmanExport, String> {
        // -----------------------------------------------------------------------
        // 1. Fetch the root collection and guard against folder exports.
        // -----------------------------------------------------------------------
        let root = collection_svc::get_collection(db.clone(), collection_id.clone())
            .await?
            .ok_or_else(|| format!("Collection '{collection_id}' not found."))?;

        if root.parent_id.is_some() {
            return Err(
                "Only top-level collections can be exported. \
                 Folders (sub-collections) cannot be exported individually."
                    .to_string(),
            );
        }

        // -----------------------------------------------------------------------
        // 2. Load the full collection tree (all collections + requests).
        // -----------------------------------------------------------------------
        let all_collections = collection_svc::list_collections(db.clone()).await?;

        // Collect the IDs of every collection node in this tree so we can
        // filter history and saved responses.
        let mut tree_collection_ids: Vec<String> = Vec::new();
        Self::collect_tree_ids(&collection_id, &all_collections, &mut tree_collection_ids);

        // -----------------------------------------------------------------------
        // 3. Build the CollectionNode tree recursively.
        // -----------------------------------------------------------------------
        let collection_node = Self::build_node(
            db.clone(),
            &root,
            &all_collections,
        )
        .await?;

        // -----------------------------------------------------------------------
        // 4. Collect history entries that belong to this collection tree.
        // -----------------------------------------------------------------------
        let all_history = history_svc::list_entries(db.clone(), None).await?;
        let history: Vec<HistoryEntry> = all_history
            .into_iter()
            .filter(|e| tree_collection_ids.contains(&e.request.collection_id))
            .collect();

        // -----------------------------------------------------------------------
        // 5. Collect all environments with their variables.
        // -----------------------------------------------------------------------
        let env_list = env_svc::list_environments(db.clone()).await?;
        let mut environments: Vec<EnvironmentExport> = Vec::new();
        for env in env_list {
            let variables = env_svc::list_variables(db.clone(), env.id.clone()).await?;
            environments.push(EnvironmentExport { environment: env, variables });
        }

        // -----------------------------------------------------------------------
        // 6. Assemble the envelope.
        // -----------------------------------------------------------------------
        Ok(LightmanExport {
            lightman_version: LIGHTMAN_FORMAT_VERSION.to_string(),
            exported_at: Utc::now().to_rfc3339(),
            collection: collection_node,
            history,
            environments,
        })
    }

    /// Serialize a [`LightmanExport`] to a pretty-printed JSON string.
    pub fn to_json(export: &LightmanExport) -> Result<String, String> {
        serde_json::to_string_pretty(export)
            .map_err(|e| format!("Failed to serialize export to JSON: {e}"))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Collect the IDs of all collections in the tree rooted at `root_id`.
    fn collect_tree_ids(
        root_id: &str,
        all: &[crate::models::Collection],
        out: &mut Vec<String>,
    ) {
        out.push(root_id.to_string());
        for col in all {
            if col.parent_id.as_deref() == Some(root_id) {
                Self::collect_tree_ids(&col.id, all, out);
            }
        }
    }

    /// Recursively build a `CollectionNode` for `col` by fetching its requests
    /// and saved responses, then recursing into sub-collections.
    async fn build_node(
        db: DbHandle,
        col: &Collection,
        all_collections: &[Collection],
    ) -> Result<CollectionNode, String> {
        // Requests in this collection, ordered by position.
        let requests = collection_svc::list_requests(db.clone(), col.id.clone()).await?;

        // Saved responses for each request in this collection.
        let mut saved_responses: Vec<SavedResponseGroup> = Vec::new();
        for req in &requests {
            let resps = saved_response_svc::list_saved_responses(db.clone(), req.id.clone()).await?;
            if !resps.is_empty() {
                saved_responses.push(SavedResponseGroup {
                    request_id: req.id.clone(),
                    responses: resps,
                });
            }
        }

        // Sub-collections, ordered by their position field.
        let mut children: Vec<&Collection> = all_collections
            .iter()
            .filter(|c| c.parent_id.as_deref() == Some(&col.id))
            .collect();
        children.sort_by_key(|c| c.position);

        // Recurse.
        let mut sub_collections: Vec<CollectionNode> = Vec::new();
        for child in children {
            sub_collections.push(
                Box::pin(Self::build_node(db.clone(), child, all_collections)).await?,
            );
        }

        Ok(CollectionNode {
            collection: col.clone(),
            requests,
            sub_collections,
            saved_responses,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, HttpMethod, RequestBody};
    use crate::storage;
    use uuid::Uuid;

    fn test_db() -> (DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_export_v2_{}.db", Uuid::new_v4()));
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

    #[test]
    fn export_nonexistent_collection_returns_error() {
        let (db, _) = test_db();
        rt().block_on(async {
            let result = ExportService::export_collection(db, "no-such-id".to_string()).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("not found"));
        });
    }

    #[test]
    fn export_folder_returns_error() {
        let (db, _) = test_db();
        rt().block_on(async {
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

            let result = ExportService::export_collection(db, folder.id).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("top-level"));
        });
    }

    #[test]
    fn export_includes_sub_collections_and_requests() {
        let (db, _) = test_db();
        rt().block_on(async {
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
                db.clone(), root.id.clone(), "Root Req".to_string(),
                HttpMethod::Get, "https://example.com".to_string(),
                vec![], vec![], RequestBody::None, AuthConfig::None,
            )
            .await
            .unwrap();

            collection_svc::create_request(
                db.clone(), folder.id.clone(), "Folder Req".to_string(),
                HttpMethod::Post, "https://example.com/sub".to_string(),
                vec![], vec![], RequestBody::None, AuthConfig::None,
            )
            .await
            .unwrap();

            let export = ExportService::export_collection(db, root.id).await.unwrap();

            assert_eq!(export.lightman_version, "2.0");
            assert_eq!(export.collection.requests.len(), 1);
            assert_eq!(export.collection.sub_collections.len(), 1);
            assert_eq!(export.collection.sub_collections[0].requests.len(), 1);
        });
    }

    #[test]
    fn export_includes_environments() {
        let (db, _) = test_db();
        rt().block_on(async {
            let root = collection_svc::create_collection(
                db.clone(), "Root".to_string(), None, None, AuthConfig::None,
            )
            .await
            .unwrap();

            env_svc::create_environment(db.clone(), "Production".to_string())
                .await
                .unwrap();

            let export = ExportService::export_collection(db, root.id).await.unwrap();
            assert_eq!(export.environments.len(), 1);
            assert_eq!(export.environments[0].environment.name, "Production");
        });
    }

    #[test]
    fn to_json_produces_valid_json_with_required_keys() {
        let (db, _) = test_db();
        rt().block_on(async {
            let col = collection_svc::create_collection(
                db.clone(), "JSON Test".to_string(), None, None, AuthConfig::None,
            )
            .await
            .unwrap();

            let export = ExportService::export_collection(db, col.id).await.unwrap();
            let json = ExportService::to_json(&export).unwrap();

            let parsed: serde_json::Value = serde_json::from_str(&json)
                .expect("to_json produced invalid JSON");

            assert!(parsed.get("lightman_version").is_some());
            assert!(parsed.get("exported_at").is_some());
            assert!(parsed.get("collection").is_some());
            assert!(parsed.get("history").is_some());
            assert!(parsed.get("environments").is_some());
            assert_eq!(parsed["lightman_version"].as_str().unwrap(), "2.0");
        });
    }
}
