// Tauri IPC commands for Environment, Variable, and Global Variable management.
// These are the entry points called by the React frontend via invoke().
//
// Task 8.2: Expose all EnvironmentService functions as #[tauri::command] async functions.
//
// Requirements: 3.1, 3.6, 8.3

use crate::models::{Environment, Variable};
use crate::services::environment as svc;
use crate::storage::DbHandle;
use tauri::State;

// ---------------------------------------------------------------------------
// Environment commands
// ---------------------------------------------------------------------------

/// Create a new environment.
///
/// The first environment created is automatically activated.
/// Subsequent ones are inactive by default.
///
/// # Frontend call
/// ```ts
/// invoke<Environment>('create_environment', { name })
/// ```
#[tauri::command]
pub async fn create_environment(
    db: State<'_, DbHandle>,
    name: String,
) -> Result<Environment, String> {
    svc::create_environment(db.inner().clone(), name).await
}

/// Return all environments ordered by name.
///
/// # Frontend call
/// ```ts
/// invoke<Environment[]>('list_environments')
/// ```
#[tauri::command]
pub async fn list_environments(db: State<'_, DbHandle>) -> Result<Vec<Environment>, String> {
    svc::list_environments(db.inner().clone()).await
}

/// Update the display name of an existing environment.
///
/// Returns an error if the environment does not exist or the name is invalid.
///
/// # Frontend call
/// ```ts
/// invoke<Environment>('update_environment', { id, name })
/// ```
#[tauri::command]
pub async fn update_environment(
    db: State<'_, DbHandle>,
    id: String,
    name: String,
) -> Result<Environment, String> {
    svc::update_environment(db.inner().clone(), id, name).await
}

/// Delete an environment and all of its variables.
///
/// Returns an error if the environment does not exist.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_environment', { id })
/// ```
#[tauri::command]
pub async fn delete_environment(db: State<'_, DbHandle>, id: String) -> Result<(), String> {
    svc::delete_environment(db.inner().clone(), id).await
}

/// Set a single environment as active, deactivating all others.
///
/// Returns the newly activated `Environment`.
///
/// # Frontend call
/// ```ts
/// invoke<Environment>('set_active_environment', { id })
/// ```
#[tauri::command]
pub async fn set_active_environment(
    db: State<'_, DbHandle>,
    id: String,
) -> Result<Environment, String> {
    svc::set_active_environment(db.inner().clone(), id).await
}

/// Deactivate all environments so that no environment is active ("No environment" state).
///
/// # Frontend call
/// ```ts
/// invoke<void>('clear_active_environment')
/// ```
#[tauri::command]
pub async fn clear_active_environment(db: State<'_, DbHandle>) -> Result<(), String> {
    svc::clear_active_environment(db.inner().clone()).await
}

// ---------------------------------------------------------------------------
// Variable commands
// ---------------------------------------------------------------------------

/// Create a new variable inside an environment.
///
/// Pass `environment_id = null` to create a **global** variable.
///
/// # Frontend call
/// ```ts
/// invoke<Variable>('create_variable', { environmentId, key, value, enabled })
/// ```
#[tauri::command]
pub async fn create_variable(
    db: State<'_, DbHandle>,
    environment_id: Option<String>,
    key: String,
    value: String,
    enabled: bool,
) -> Result<Variable, String> {
    svc::create_variable(db.inner().clone(), environment_id, key, value, enabled).await
}

/// Return all variables belonging to the given environment.
///
/// # Frontend call
/// ```ts
/// invoke<Variable[]>('list_variables', { environmentId })
/// ```
#[tauri::command]
pub async fn list_variables(
    db: State<'_, DbHandle>,
    environment_id: String,
) -> Result<Vec<Variable>, String> {
    svc::list_variables(db.inner().clone(), environment_id).await
}

/// Update the key, value, and enabled state of an existing variable.
///
/// Returns an error if the variable does not exist.
///
/// # Frontend call
/// ```ts
/// invoke<Variable>('update_variable', { id, key, value, enabled })
/// ```
#[tauri::command]
pub async fn update_variable(
    db: State<'_, DbHandle>,
    id: String,
    key: String,
    value: String,
    enabled: bool,
) -> Result<Variable, String> {
    svc::update_variable(db.inner().clone(), id, key, value, enabled).await
}

/// Delete a variable by its ID.
///
/// Returns an error if the variable does not exist.
///
/// # Frontend call
/// ```ts
/// invoke<void>('delete_variable', { id })
/// ```
#[tauri::command]
pub async fn delete_variable(db: State<'_, DbHandle>, id: String) -> Result<(), String> {
    svc::delete_variable(db.inner().clone(), id).await
}

/// Return all **global** variables (those with `environment_id` = null).
///
/// # Frontend call
/// ```ts
/// invoke<Variable[]>('list_global_variables')
/// ```
#[tauri::command]
pub async fn list_global_variables(db: State<'_, DbHandle>) -> Result<Vec<Variable>, String> {
    svc::list_global_variables(db.inner().clone()).await
}
