//! Immutable public-file snapshot: no request-time filesystem access or traversal.
use axum::{
    body::Bytes,
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{collections::BTreeMap, path::Path};

/// Optional legacy static/endpoint content, independent of admin build assets.
#[derive(Default)]
pub struct Assets(BTreeMap<String, (Bytes, &'static str)>);
impl Assets {
    /// Load ordinary, non-hidden files with bounded memory and no symlink traversal.
    pub fn load(root: &Path) -> Result<Self, std::io::Error> {
        if !root.exists() {
            return Ok(Self::default());
        }
        let mut result = Self::default();
        let mut total = 0;
        result.collect(root, "", &mut total)?;
        Ok(result)
    }
    fn collect(
        &mut self,
        path: &Path,
        prefix: &str,
        total: &mut u64,
    ) -> Result<(), std::io::Error> {
        if path.symlink_metadata()?.file_type().is_symlink() {
            return Err(std::io::Error::other(
                "Public assets must not contain symbolic links",
            ));
        }
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| std::io::Error::other("Public asset path is not UTF-8"))?;
            if name.starts_with('.') {
                continue;
            }
            if name.contains(['\\', '?', '#', '%']) {
                return Err(std::io::Error::other("Public asset path is unsupported"));
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(std::io::Error::other(
                    "Public assets must not contain symbolic links",
                ));
            }
            let route = format!("{prefix}/{name}");
            if kind.is_dir() {
                if route.matches('/').count() > 16 {
                    return Err(std::io::Error::other(
                        "Public asset nesting exceeds its limit",
                    ));
                }
                self.collect(&entry.path(), &route, total)?;
                continue;
            }
            if !kind.is_file() {
                return Err(std::io::Error::other(
                    "Public assets must be ordinary files",
                ));
            }
            let size = entry.metadata()?.len();
            *total += size;
            if size > 8 * 1024 * 1024 || *total > 32 * 1024 * 1024 || self.0.len() >= 1000 {
                return Err(std::io::Error::other(
                    "Public assets exceed their size or count limit",
                ));
            }
            let mime = match entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "ico" => "image/x-icon",
                "svg" => "image/svg+xml",
                "css" => "text/css; charset=utf-8",
                "js" => "text/javascript; charset=utf-8",
                "html" | "htm" => "text/html; charset=utf-8",
                "txt" => "text/plain; charset=utf-8",
                _ => "application/octet-stream",
            };
            self.0.insert(
                format!("/static{route}"),
                (std::fs::read(entry.path())?.into(), mime),
            );
        }
        Ok(())
    }
    pub(crate) fn response(&self, path: &str, method: &Method) -> Response {
        if !matches!(*method, Method::GET | Method::HEAD) {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        let Some((body, mime)) = self.0.get(path) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        ([(header::CONTENT_TYPE,*mime),(header::X_CONTENT_TYPE_OPTIONS,"nosniff"),(header::REFERRER_POLICY,"no-referrer"),(header::CACHE_CONTROL,"no-store"),(header::CONTENT_SECURITY_POLICY,"sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; base-uri 'none'; frame-ancestors 'none'")],if *method==Method::HEAD {Bytes::new()} else {body.clone()}).into_response()
    }
}
