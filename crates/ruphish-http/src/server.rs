//! Startup routers; business routes arrive with their owning feature packages.

use axum::{
    Router,
    body::{Body, Bytes},
    http::{HeaderMap, Request, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tower_http::compression::CompressionLayer;

/// Validate and mount an explicit set of built admin assets without SPA catch-all routing.
///
/// Symlinks, hidden files, API namespaces and non-asset extensions are rejected.
pub fn admin_router(directory: &Path) -> Result<AdminAssets, std::io::Error> {
    let root = directory.canonicalize()?;
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    let manifest: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(root.join("asset-manifest.json"))?)
            .map_err(std::io::Error::other)?;
    if manifest.len() + 1 != files.len() {
        return Err(std::io::Error::other(
            "Admin asset manifest does not match the distribution; rebuild with Trunk",
        ));
    }
    let mut snapshots = Vec::new();
    for (name, path) in &files {
        let bytes = std::fs::read(path)?;
        let digest = hex::encode(Sha256::digest(&bytes));
        if name != "asset-manifest.json" && manifest.get(name) != Some(&digest) {
            return Err(std::io::Error::other(format!(
                "Admin asset integrity failed for {name}; rebuild with Trunk"
            )));
        }
        snapshots.push((name.clone(), Bytes::from(bytes), format!("\"{digest}\"")));
    }
    let names: BTreeSet<_> = files.iter().map(|(name, _)| name.as_str()).collect();
    for name in [
        "index.html",
        "visual-frame.js",
        "visual-frame.css",
        "THIRD-PARTY-NOTICES.txt",
    ] {
        if !names.contains(name) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Missing required admin asset: {name}"),
            ));
        }
    }
    if !names.iter().any(|name| name.ends_with(".wasm")) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Missing compiled admin WASM",
        ));
    }
    let mut index = String::new();
    let mut router = Router::new();
    for (name, bytes, etag) in snapshots {
        if name == "index.html" {
            index = String::from_utf8(bytes.to_vec()).map_err(std::io::Error::other)?;
            continue;
        }
        let route = format!("/{name}");
        let mime = content_type(&name);
        router = router.route(
            &route,
            get(move |headers: HeaderMap| {
                let bytes = bytes.clone();
                let etag = etag.clone();
                async move { asset(bytes, mime, &etag, &headers) }
            }),
        );
    }
    Ok(AdminAssets { router, index })
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

fn asset(bytes: Bytes, mime: &'static str, etag: &str, headers: &HeaderMap) -> Response {
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|tag| matches!(tag.trim(), "*") || tag.trim().trim_start_matches("W/") == etag)
        })
    {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    let size = bytes.len();
    let mut status = StatusCode::OK;
    let mut body = bytes;
    let mut range = None;
    if headers
        .get(header::IF_RANGE)
        .is_none_or(|value| value == etag)
        && let Some(value) = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("bytes="))
        && !value.contains(',')
    {
        let bounds = value.split_once('-').and_then(|(first, last)| {
            if first.is_empty() {
                let suffix = last.parse::<usize>().ok()?;
                (suffix > 0 && size > 0)
                    .then_some((size.saturating_sub(suffix), size.saturating_sub(1)))
            } else {
                let first = first.parse::<usize>().ok()?;
                let last = if last.is_empty() {
                    size.saturating_sub(1)
                } else {
                    last.parse::<usize>().ok()?.min(size.saturating_sub(1))
                };
                (first <= last && first < size).then_some((first, last))
            }
        });
        let Some((first, last)) = bounds else {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response();
        };
        status = StatusCode::PARTIAL_CONTENT;
        body = body.slice(first..=last);
        range = Some(format!("bytes {first}-{last}/{size}"));
    }
    let mut response = (
        status,
        [
            (header::CONTENT_TYPE, mime),
            (header::ETAG, etag),
            (header::ACCEPT_RANGES, "bytes"),
        ],
        body,
    )
        .into_response();
    if let Some(range) = range.and_then(|s| s.parse().ok()) {
        response.headers_mut().insert(header::CONTENT_RANGE, range);
    }
    response
}

/// Validated generated assets, composed with authenticated application routes at startup.
pub struct AdminAssets {
    pub(crate) router: Router,
    pub(crate) index: String,
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
    router
        .fallback(|| async { super::response::not_found() })
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(access_log))
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

fn collect(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.path().symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Admin asset symlinks are not permitted",
            ));
        }
        let path = entry.path();
        let name = path
            .strip_prefix(root)
            .map_err(std::io::Error::other)?
            .to_string_lossy()
            .replace('\\', "/");
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
            || name.split('/').any(|part| part.starts_with('.'))
            || name == "api"
            || name.starts_with("api/")
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Admin assets contain an unsupported path: {name:?}; rebuild with Trunk"),
            ));
        }
        if metadata.is_dir() {
            collect(root, &path, files)?;
        } else if metadata.is_file() {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if !matches!(
                extension,
                "html"
                    | "js"
                    | "mjs"
                    | "css"
                    | "wasm"
                    | "txt"
                    | "json"
                    | "svg"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "ico"
                    | "woff"
                    | "woff2"
                    | "map"
            ) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Admin assets contain a non-asset file",
                ));
            }
            files.push((name, path));
        }
    }
    Ok(())
}
