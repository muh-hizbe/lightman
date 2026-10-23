// Lightman — Rust API Client Desktop Application
// Built with Tauri 2.0 + React/TypeScript frontend.

pub mod commands;
pub mod models;
pub mod services;
pub mod storage;

use tauri::Manager;

use commands::collection::{
    create_collection, delete_collection, get_collection, list_collections, update_collection,
    create_request, delete_request, duplicate_request, get_request, list_requests,
    move_request, reorder_request, update_request,
};

use commands::environment::{
    create_environment, list_environments, update_environment, delete_environment,
    set_active_environment, clear_active_environment, create_variable, list_variables,
    update_variable, delete_variable, list_global_variables,
};

use commands::http::{cancel_request, send_request};

use commands::history::{
    list_history_entries, search_history_entries, delete_history_entry, clear_history,
};

use commands::export::export_collection;
use commands::import::import_collection;
use commands::sample::list_samples;
use commands::saved_response::{
    save_response, list_saved_responses, rename_saved_response, delete_saved_response,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Resolve the database path using Tauri 2's PathResolver:
            // <App Data Dir>/lightman.db
            let db_path = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir")
                .join("lightman.db");

            // Ensure the parent directory exists.
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let db = storage::initialize_database(db_path)
                .map_err(|e| format!("Failed to initialise database: {e}"))?;

            app.manage(db);
            app.manage(commands::http::CancelHandle(std::sync::Mutex::new(None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Collection commands (Task 7.2)
            create_collection,
            get_collection,
            list_collections,
            update_collection,
            delete_collection,
            // Request commands (Task 7.2)
            create_request,
            get_request,
            list_requests,
            update_request,
            delete_request,
            duplicate_request,
            move_request,
            reorder_request,
            // Environment commands (Task 8.2)
            create_environment,
            list_environments,
            update_environment,
            delete_environment,
            set_active_environment,
            clear_active_environment,
            // Variable commands (Task 8.2)
            create_variable,
            list_variables,
            update_variable,
            delete_variable,
            list_global_variables,
            // HTTP command (Task 9.1)
            send_request,
            cancel_request,
            // History commands (Task 10.2)
            list_history_entries,
            search_history_entries,
            delete_history_entry,
            clear_history,
            // Export/Import commands (Task 11.4)
            export_collection,
            import_collection,
            // Sample commands (Task 12.2)
            list_samples,
            // Saved response commands
            save_response,
            list_saved_responses,
            rename_saved_response,
            delete_saved_response,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
