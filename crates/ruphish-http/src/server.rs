//! Administration asset routers, admission control and request logging.

use crate::static_asset::Asset;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use std::path::Path;

/// Validate and mount an explicit set of built admin assets without SPA catch-all routing.
///
/// Symlinks, hidden files, API namespaces and non-asset extensions are rejected.
pub fn admin_router(directory: &Path) -> Result<AdminAssets, std::io::Error> {
    mount(
        crate::asset_distribution::load(directory)?
            .into_iter()
            .map(|(name, bytes)| {
                let asset = Asset::identity(bytes.into());
                (name, asset)
            }),
    )
}

/// Mount the validated distribution compiled into a standalone executable.
#[cfg(feature = "embedded-assets")]
pub fn embedded_admin_router() -> Result<AdminAssets, std::io::Error> {
    mount(include!(concat!(env!("OUT_DIR"), "/admin_assets.rs")))
}

fn mount(assets: impl IntoIterator<Item = (String, Asset)>) -> Result<AdminAssets, std::io::Error> {
    let mut index = None;
    let mut router = Router::new();
    for (name, asset) in assets {
        if name == "index.html" {
            index = Some(asset);
            continue;
        }
        let route = format!("/{name}");
        let mime = content_type(&name);
        let cache = if content_hashed(&name) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        router = router.route(
            &route,
            get(move |method: axum::http::Method, headers: HeaderMap| {
                let asset = asset.clone();
                async move { asset.response(mime, cache, &method, &headers) }
            }),
        );
    }
    Ok(AdminAssets {
        router,
        index: index.ok_or_else(|| std::io::Error::other("Missing admin entry document"))?,
    })
}

fn content_hashed(name: &str) -> bool {
    // Trunk names root assets <stem>-<16 hex digits>[ _bg ].<extension>.
    if name.contains('/') {
        return false;
    }
    name.rsplit_once('.')
        .and_then(|(stem, _)| stem.strip_suffix("_bg").unwrap_or(stem).rsplit_once('-'))
        .is_some_and(|(_, hash)| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "wasm" => "application/wasm",
        "txt" => "text/plain",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Validated generated assets, composed with authenticated application routes at startup.
pub struct AdminAssets {
    pub(crate) router: Router,
    pub(crate) index: Asset,
}

/// Bound admission and the entire request, including extraction of a slow body.
pub(crate) async fn admin_admission(
    axum::extract::State(capacity): axum::extract::State<std::sync::Arc<tokio::sync::Semaphore>>,
    request: Request<Body>,
    next: middleware::Next,
) -> Response {
    let Ok(_permit) = capacity.try_acquire() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONNECTION, "close"), (header::RETRY_AFTER, "1")],
        )
            .into_response();
    };
    match tokio::time::timeout(std::time::Duration::from_secs(30), next.run(request)).await {
        Ok(response) => response,
        Err(_) => (StatusCode::REQUEST_TIMEOUT, [(header::CONNECTION, "close")]).into_response(),
    }
}

pub(crate) fn finish(router: Router) -> Router {
    router.layer(middleware::from_fn(access_log))
}

/// Public listener with the existing unknown-route contract; no admin application exposure.
pub fn public_router() -> Router {
    Router::new()
        .fallback(|| async { super::response::not_found() })
        .layer(middleware::from_fn(access_log))
}

pub(crate) async fn access_log(
    request: Request<Body>,
    next: middleware::Next,
) -> axum::response::Response {
    // Only router-owned patterns are safe. Attackers can put credentials in a
    // path, query, header, body, or even an extension HTTP method.
    let method = match request.method().as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        "PATCH" => "PATCH",
        "OPTIONS" => "OPTIONS",
        _ => "OTHER",
    };
    let path = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| "<unmatched>".into());
    let response = next.run(request).await;
    tracing::info!(%method, %path, status = response.status().as_u16(), "http_request");
    response
}
