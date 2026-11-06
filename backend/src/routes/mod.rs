mod auth;
mod health;
mod users;

use std::path::Path;

use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{error::AppError, state::AppState};

pub fn router(state: AppState, static_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::routes())
        .nest("/users", users::routes())
        .fallback(|| async { AppError::NotFound });

    let mut app = Router::new().nest("/api", api);

    // The frontend is a single-page app: unknown paths get index.html
    // and the client-side router takes over.
    if let Some(dir) = static_dir {
        let spa = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
        app = app
            .fallback_service(spa)
            .layer(middleware::from_fn(static_cache_headers));
    }

    app.layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Hashed build assets never change and can be cached forever. Everything
/// else (index.html, sw.js, manifest) must be revalidated, otherwise phones
/// keep running an old version after an update.
async fn static_cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let value = if path.starts_with("/api/") {
        None
    } else if path.starts_with("/_app/immutable/") {
        Some("public, max-age=31536000, immutable")
    } else {
        Some("no-cache")
    };
    let mut res = next.run(req).await;
    if let Some(v) = value {
        res.headers_mut()
            .entry(header::CACHE_CONTROL)
            .or_insert(HeaderValue::from_static(v));
    }
    res
}
