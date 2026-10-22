// Environment and Variable service — CRUD, active environment management, global variables.
// Task 8.1: Full CRUD for environments and variables with set_active_environment logic.
//
// Requirements: 3.1, 3.3, 3.6

use crate::models::{Environment, Variable};
use crate::storage::{self, DbHandle, EnvironmentRow, VariableRow};
use chrono::Utc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate that `name` is non-empty and at most 255 characters.
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

fn environment_from_row(row: EnvironmentRow) -> Environment {
    Environment {
        id: row.id,
        name: row.name,
        is_active: row.is_active,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn variable_from_row(row: VariableRow) -> Variable {
    Variable {
        id: row.id,
        environment_id: row.environment_id,
        key: row.key,
        value: row.value,
        enabled: row.enabled,
    }
}

// ---------------------------------------------------------------------------
// Environment operations (Requirements 3.1, 3.6)
// ---------------------------------------------------------------------------

/// Create a new environment.
///
/// The first environment created is automatically activated.
/// Subsequent ones are inactive by default.
pub async fn create_environment(db: DbHandle, name: String) -> Result<Environment, String> {
    validate_name(&name)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Auto-activate if this is the very first environment.
    let existing = storage::list_environments(db.clone()).await?;
    let is_active = existing.is_empty();

    storage::insert_environment(db.clone(), id.clone(), name, is_active, now.clone(), now).await?;

    let row = storage::get_active_environment(db.clone())
        .await?
        // If it isn't active, fall back to listing and finding by id.
        .filter(|r| r.id == id);

    let row = match row {
        Some(r) => r,
        None => {
            // Fetch by listing since there's no get_environment_by_id helper.
            storage::list_environments(db)
                .await?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or_else(|| "Environment was not found immediately after insert.".to_string())?
        }
    };

    Ok(environment_from_row(row))
}

/// Return all environments ordered by name.
pub async fn list_environments(db: DbHandle) -> Result<Vec<Environment>, String> {
    let rows = storage::list_environments(db).await?;
    Ok(rows.into_iter().map(environment_from_row).collect())
}

/// Update the display name of an existing environment.
///
/// Returns `Err` if the environment does not exist or the name is invalid.
pub async fn update_environment(
    db: DbHandle,
    id: String,
    name: String,
) -> Result<Environment, String> {
    validate_name(&name)?;

    // Verify existence and fetch current is_active state.
    let existing = storage::list_environments(db.clone())
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Environment '{id}' not found."))?;

    let now = Utc::now().to_rfc3339();
    storage::update_environment(db.clone(), id.clone(), name, existing.is_active, now).await?;

    let row = storage::list_environments(db)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Environment '{id}' not found after update."))?;

    Ok(environment_from_row(row))
}

/// Delete an environment and all of its variables (via ON DELETE CASCADE).
///
/// If the deleted environment was active, no environment is left active —
/// callers should invoke `set_active_environment` to activate another one if
/// needed.
///
/// Returns `Err` if the environment does not exist.
pub async fn delete_environment(db: DbHandle, id: String) -> Result<(), String> {
    // Verify existence.
    storage::list_environments(db.clone())
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Environment '{id}' not found."))?;

    storage::delete_environment(db, id).await
}

/// Deactivate all environments so that no environment is active.
///
/// This is the "No environment" state — variable substitution will use only
/// global variables when no environment is active.
pub async fn clear_active_environment(db: DbHandle) -> Result<(), String> {
    let all = storage::list_environments(db.clone()).await?;
    let now = Utc::now().to_rfc3339();

    for env in &all {
        if env.is_active {
            storage::update_environment(
                db.clone(),
                env.id.clone(),
                env.name.clone(),
                false,
                now.clone(),
            )
            .await?;
        }
    }

    Ok(())
}

/// Set a single environment as active, deactivating all others.
///
/// This is an atomic operation: all environments are deactivated first, then
/// the selected one is activated.
///
/// Returns the newly activated `Environment`.
pub async fn set_active_environment(db: DbHandle, id: String) -> Result<Environment, String> {
    // Load all environments and verify the target exists.
    let all = storage::list_environments(db.clone()).await?;
    let target = all
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Environment '{id}' not found."))?
        .clone();

    let now = Utc::now().to_rfc3339();

    // Deactivate every environment that is currently active (except the target,
    // which we will activate in the next step).
    for env in &all {
        if env.is_active && env.id != id {
            storage::update_environment(
                db.clone(),
                env.id.clone(),
                env.name.clone(),
                false,
                now.clone(),
            )
            .await?;
        }
    }

    // Activate the target.
    storage::update_environment(db.clone(), id.clone(), target.name.clone(), true, now).await?;

    // Return the freshly activated environment.
    let row = storage::list_environments(db)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Environment '{id}' not found after activation."))?;

    Ok(environment_from_row(row))
}

// ---------------------------------------------------------------------------
// Variable operations (Requirements 3.1, 3.3)
// ---------------------------------------------------------------------------

/// Create a new variable inside an environment.
///
/// Pass `environment_id = None` to create a **global** variable.
pub async fn create_variable(
    db: DbHandle,
    environment_id: Option<String>,
    key: String,
    value: String,
    enabled: bool,
) -> Result<Variable, String> {
    if key.is_empty() {
        return Err("Variable key must not be empty.".to_string());
    }

    // If environment_id is provided, verify the environment exists.
    if let Some(ref env_id) = environment_id {
        let exists = storage::list_environments(db.clone())
            .await?
            .iter()
            .any(|r| &r.id == env_id);
        if !exists {
            return Err(format!("Environment '{env_id}' not found."));
        }
    }

    let id = Uuid::new_v4().to_string();
    storage::insert_variable(db.clone(), id.clone(), environment_id.clone(), key, value, enabled)
        .await?;

    // Fetch back the inserted row.
    let row = match &environment_id {
        Some(env_id) => storage::list_variables_by_env(db, env_id.clone())
            .await?
            .into_iter()
            .find(|r| r.id == id),
        None => storage::list_global_variables(db)
            .await?
            .into_iter()
            .find(|r| r.id == id),
    }
    .ok_or_else(|| "Variable was not found immediately after insert.".to_string())?;

    Ok(variable_from_row(row))
}

/// Return all variables belonging to the given environment.
pub async fn list_variables(db: DbHandle, environment_id: String) -> Result<Vec<Variable>, String> {
    let rows = storage::list_variables_by_env(db, environment_id).await?;
    Ok(rows.into_iter().map(variable_from_row).collect())
}

/// Update the key, value, and enabled state of an existing variable.
///
/// Returns `Err` if the variable does not exist.
pub async fn update_variable(
    db: DbHandle,
    id: String,
    key: String,
    value: String,
    enabled: bool,
) -> Result<Variable, String> {
    if key.is_empty() {
        return Err("Variable key must not be empty.".to_string());
    }

    storage::update_variable(db.clone(), id.clone(), key, value, enabled).await?;

    // Fetch the updated row — we don't know its environment_id here, so we
    // use the generic lookup helper.
    let row = storage::get_variable_by_id(db, id.clone())
        .await?
        .ok_or_else(|| format!("Variable '{id}' not found after update."))?;

    Ok(variable_from_row(row))
}

/// Delete a variable by its ID.
///
/// Returns `Err` if the variable does not exist.
pub async fn delete_variable(db: DbHandle, id: String) -> Result<(), String> {
    // Verify existence.
    storage::get_variable_by_id(db.clone(), id.clone())
        .await?
        .ok_or_else(|| format!("Variable '{id}' not found."))?;

    storage::delete_variable(db, id).await
}

/// Return all **global** variables (those with `environment_id IS NULL`).
pub async fn list_global_variables(db: DbHandle) -> Result<Vec<Variable>, String> {
    let rows = storage::list_global_variables(db).await?;
    Ok(rows.into_iter().map(variable_from_row).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    fn test_db() -> (DbHandle, std::path::PathBuf) {
        let path = std::env::temp_dir()
            .join(format!("lightman_svc_env_{}.db", Uuid::new_v4()));
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

    // ---- Environment CRUD ----

    #[test]
    fn create_environment_succeeds() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db, "Development".to_string())
                .await
                .unwrap();
            assert!(!env.id.is_empty());
            assert_eq!(env.name, "Development");
        });
    }

    #[test]
    fn first_environment_is_auto_activated() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db, "Production".to_string())
                .await
                .unwrap();
            assert!(env.is_active);
        });
    }

    #[test]
    fn second_environment_is_not_auto_activated() {
        let (db, _path) = test_db();
        rt().block_on(async {
            create_environment(db.clone(), "First".to_string())
                .await
                .unwrap();
            let second = create_environment(db, "Second".to_string())
                .await
                .unwrap();
            assert!(!second.is_active);
        });
    }

    #[test]
    fn create_environment_empty_name_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_environment(db, "".to_string()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn list_environments_returns_all() {
        let (db, _path) = test_db();
        rt().block_on(async {
            create_environment(db.clone(), "Dev".to_string()).await.unwrap();
            create_environment(db.clone(), "Staging".to_string()).await.unwrap();
            create_environment(db.clone(), "Prod".to_string()).await.unwrap();

            let envs = list_environments(db).await.unwrap();
            assert_eq!(envs.len(), 3);
        });
    }

    #[test]
    fn update_environment_name() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Old".to_string()).await.unwrap();
            let updated = update_environment(db, env.id.clone(), "New".to_string())
                .await
                .unwrap();
            assert_eq!(updated.id, env.id);
            assert_eq!(updated.name, "New");
        });
    }

    #[test]
    fn update_environment_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = update_environment(db, "no-such-id".to_string(), "Name".to_string()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn delete_environment_removes_it() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Temp".to_string()).await.unwrap();
            delete_environment(db.clone(), env.id.clone()).await.unwrap();

            let envs = list_environments(db).await.unwrap();
            assert!(envs.iter().all(|e| e.id != env.id));
        });
    }

    #[test]
    fn delete_environment_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = delete_environment(db, "ghost".to_string()).await;
            assert!(result.is_err());
        });
    }

    // ---- set_active_environment ----

    #[test]
    fn set_active_environment_deactivates_others() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let e1 = create_environment(db.clone(), "Dev".to_string()).await.unwrap();
            let e2 = create_environment(db.clone(), "Prod".to_string()).await.unwrap();

            // e1 should be active (first-created), e2 inactive.
            assert!(e1.is_active);
            assert!(!e2.is_active);

            // Activate e2.
            let activated = set_active_environment(db.clone(), e2.id.clone()).await.unwrap();
            assert!(activated.is_active);

            // Verify e1 is now inactive.
            let all = list_environments(db).await.unwrap();
            let e1_updated = all.iter().find(|e| e.id == e1.id).unwrap();
            let e2_updated = all.iter().find(|e| e.id == e2.id).unwrap();
            assert!(!e1_updated.is_active);
            assert!(e2_updated.is_active);
        });
    }

    #[test]
    fn set_active_environment_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = set_active_environment(db, "nonexistent".to_string()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn set_active_environment_idempotent() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Dev".to_string()).await.unwrap();
            // Activate the already-active environment — should not error.
            let result = set_active_environment(db, env.id.clone()).await;
            assert!(result.is_ok());
        });
    }

    // ---- Variable CRUD ----

    #[test]
    fn create_variable_in_environment() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Dev".to_string()).await.unwrap();

            let var = create_variable(
                db,
                Some(env.id.clone()),
                "BASE_URL".to_string(),
                "https://api.dev.example.com".to_string(),
                true,
            )
            .await
            .unwrap();

            assert!(!var.id.is_empty());
            assert_eq!(var.environment_id, Some(env.id));
            assert_eq!(var.key, "BASE_URL");
            assert_eq!(var.value, "https://api.dev.example.com");
            assert!(var.enabled);
        });
    }

    #[test]
    fn create_global_variable() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let var = create_variable(
                db,
                None,
                "GLOBAL_KEY".to_string(),
                "global_value".to_string(),
                true,
            )
            .await
            .unwrap();

            assert!(var.environment_id.is_none());
        });
    }

    #[test]
    fn create_variable_empty_key_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_variable(db, None, "".to_string(), "val".to_string(), true).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn create_variable_nonexistent_env_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = create_variable(
                db,
                Some("no-such-env".to_string()),
                "KEY".to_string(),
                "val".to_string(),
                true,
            )
            .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn list_variables_returns_env_variables() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Dev".to_string()).await.unwrap();

            for i in 0..3 {
                create_variable(
                    db.clone(),
                    Some(env.id.clone()),
                    format!("KEY_{i}"),
                    format!("val_{i}"),
                    true,
                )
                .await
                .unwrap();
            }

            let vars = list_variables(db, env.id).await.unwrap();
            assert_eq!(vars.len(), 3);
        });
    }

    #[test]
    fn list_global_variables_returns_only_globals() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Dev".to_string()).await.unwrap();

            // One env-scoped variable.
            create_variable(
                db.clone(),
                Some(env.id.clone()),
                "ENV_KEY".to_string(),
                "env_val".to_string(),
                true,
            )
            .await
            .unwrap();

            // Two global variables.
            create_variable(db.clone(), None, "GLOBAL_1".to_string(), "v1".to_string(), true)
                .await
                .unwrap();
            create_variable(db.clone(), None, "GLOBAL_2".to_string(), "v2".to_string(), true)
                .await
                .unwrap();

            let globals = list_global_variables(db).await.unwrap();
            assert_eq!(globals.len(), 2);
            assert!(globals.iter().all(|v| v.environment_id.is_none()));
        });
    }

    #[test]
    fn update_variable_changes_fields() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let var = create_variable(
                db.clone(),
                None,
                "KEY".to_string(),
                "old_value".to_string(),
                true,
            )
            .await
            .unwrap();

            let updated =
                update_variable(db, var.id.clone(), "NEW_KEY".to_string(), "new_value".to_string(), false)
                    .await
                    .unwrap();

            assert_eq!(updated.id, var.id);
            assert_eq!(updated.key, "NEW_KEY");
            assert_eq!(updated.value, "new_value");
            assert!(!updated.enabled);
        });
    }

    #[test]
    fn update_variable_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result =
                update_variable(db, "ghost".to_string(), "K".to_string(), "v".to_string(), true)
                    .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn delete_variable_removes_it() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let var = create_variable(
                db.clone(),
                None,
                "TO_DELETE".to_string(),
                "val".to_string(),
                true,
            )
            .await
            .unwrap();

            delete_variable(db.clone(), var.id.clone()).await.unwrap();

            let globals = list_global_variables(db).await.unwrap();
            assert!(globals.iter().all(|v| v.id != var.id));
        });
    }

    #[test]
    fn delete_variable_not_found_fails() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let result = delete_variable(db, "ghost".to_string()).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn delete_environment_cascades_to_variables() {
        let (db, _path) = test_db();
        rt().block_on(async {
            let env = create_environment(db.clone(), "Dev".to_string()).await.unwrap();

            create_variable(
                db.clone(),
                Some(env.id.clone()),
                "KEY".to_string(),
                "val".to_string(),
                true,
            )
            .await
            .unwrap();

            delete_environment(db.clone(), env.id.clone()).await.unwrap();

            // Variables should be gone due to ON DELETE CASCADE.
            let vars = list_variables(db, env.id).await.unwrap();
            assert!(vars.is_empty());
        });
    }
}
