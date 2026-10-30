//! Shared build-time and development-runtime distribution validation.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub fn load(directory: &Path) -> Result<Vec<(String, Vec<u8>)>, std::io::Error> {
    if directory.symlink_metadata()?.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "Admin asset symlinks are not permitted",
        ));
    }
    let root = directory.canonicalize()?;
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
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
        if name == "index.html" {
            std::str::from_utf8(&bytes).map_err(std::io::Error::other)?;
        }
        snapshots.push((name.clone(), bytes));
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
    Ok(snapshots)
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
            || name
                .split('/')
                .next()
                .is_some_and(|part| matches!(part, "api" | "auth" | "internal" | "health"))
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
        } else {
            return Err(std::io::Error::other("Admin assets must be ordinary files"));
        }
    }
    Ok(())
}
