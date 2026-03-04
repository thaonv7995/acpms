//! ACPMS Server Library
//!
//! This library exposes the server modules for use in tests.

pub mod api;
pub mod deploy_context_preparer;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod observability;
pub mod routes;
pub mod services;
pub mod ssh;
pub mod state;
pub mod types;

// Re-export commonly used types
pub use routes::create_router;
pub use state::AppState;
