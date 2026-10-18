//! Startup routers; business routes arrive with their owning feature packages.

use axum::{Router, body::Body, http::Request, middleware, routing::get_service};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tower_http::{compression::CompressionLayer, services::ServeFile};

/// Validate and mount an explicit set of built admin assets without SPA catch-all routing.
///
/// Symlinks, hidden files, API namespaces and non-asset extensions are rejected.
pub fn admin_router(directory: &Path) -> Result<Router, std::io::Error> {
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
    for (name, path) in &files {
        if name == "asset-manifest.json" {
            continue;
        }
        let digest = hex::encode(Sha256::digest(std::fs::read(path)?));
        if manifest.get(name) != Some(&digest) {
            return Err(std::io::Error::other(format!(
                "Admin asset integrity failed for {name}; rebuild with Trunk"
            )));
        }
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
    let mut router = Router::new();
    for (name, path) in files {
        let route = if name == "index.html" {
            "/".to_owned()
        } else {
            format!("/{name}")
        };
        router = router.route_service(&route, get_service(ServeFile::new(path)));
    }
    Ok(router
        .fallback(|| async { super::response::not_found() })
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(access_log)))
}

/// Public listener with the existing unknown-route contract; no admin application exposure.
pub fn public_router() -> Router {
    Router::new()
        .fallback(|| async { super::response::not_found() })
        .layer(middleware::from_fn(access_log))
}

async fn access_log(request: Request<Body>, next: middleware::Next) -> axum::response::Response {
    // Query strings can contain legacy API keys and recipient identifiers.
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
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
                "Admin assets contain an unsupported path",
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
