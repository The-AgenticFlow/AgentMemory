//! HTTP server for the Engram runtime.

pub mod routes;

use axum::Router;

pub use routes::AppState;

/// Builds the application router with all runtime endpoints.
pub fn build_app(state: AppState) -> Router {
    routes::router(state)
}
