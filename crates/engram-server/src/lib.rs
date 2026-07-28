//! HTTP server for the Engram runtime.

pub mod mcp;
pub mod routes;

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

pub use routes::AppState;

/// Builds the application router with all runtime endpoints.
pub fn build_app(state: AppState) -> Router {
    let web_root = if std::path::Path::new("web/dist/index.html").exists() {
        "web/dist"
    } else {
        "web"
    };
    let app = routes::router(state)
        .fallback_service(
            ServeDir::new(web_root)
                .not_found_service(ServeFile::new(format!("{web_root}/index.html"))),
        )
        .layer(middleware::from_fn(api_token_auth))
        .layer(cors_layer());
    tracing::warn!("[CRITICAL] Router built with fallback_service for web_root={}", web_root);
    app
}

fn cors_layer() -> CorsLayer {
    match std::env::var("ENGRAM_ALLOWED_ORIGINS") {
        Ok(origins) if !origins.trim().is_empty() && origins.trim() != "*" => {
            let origins = origins
                .split(',')
                .filter_map(|origin| origin.trim().parse::<HeaderValue>().ok())
                .collect::<Vec<_>>();
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers(Any)
                .allow_origin(origins)
        }
        _ => CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any)
            .allow_origin(Any),
    }
}

async fn api_token_auth(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let Some(expected) = std::env::var("ENGRAM_API_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return Ok(next.run(request).await);
    };
    let path = request.uri().path();
    if path == "/health" || path == "/logs" || path == "/logstest" {
        return Ok(next.run(request).await);
    }
    let authorized = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token == expected)
        .unwrap_or(false);
    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
