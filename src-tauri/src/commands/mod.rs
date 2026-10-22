// IPC Command modules — entry points from React frontend to Rust core.
// Each sub-module corresponds to a feature domain.

pub mod collection;
pub mod environment;
pub mod http;
pub mod history;
pub mod export;
pub mod import;
pub mod sample;
pub mod saved_response;
