//! HTTP server for the Engram runtime.

pub mod routes;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub use routes::AppState;

/// Builds the application router with all runtime endpoints.
pub fn build_app(state: AppState) -> Router {
    routes::router(state).fallback_service(
        ServeDir::new("web").not_found_service(ServeFile::new("web/index.html")),
    )
}
