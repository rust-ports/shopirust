//! HTTP middleware helpers for the extension preview server.

use crate::error::AppError;
use crate::services::dev::extension::payload::is_subpath;
use crate::services::dev::extension::payload::resolve_output_dir;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use std::path::{Path, PathBuf};

pub const EXTENSION_JSON_CONTENT_TYPE: &str = "application/json";

pub fn cors_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("access-control-allow-origin"),
        HeaderValue::from_static("*"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-methods"),
        HeaderValue::from_static("GET, OPTIONS"),
    );
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-cache"),
    );
    headers
}

pub fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "ico" => "image/x-icon",
        "html" | "htm" => "text/html",
        "js" | "mjs" | "cjs" => "text/javascript",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        _ => "text/plain",
    }
}

/// Resolve an asset under the extension output directory with path-jail checks.
pub fn resolve_asset_file(output_path: &Path, filesystem_path: &str) -> Result<PathBuf, AppError> {
    let output_dir = resolve_output_dir(output_path);
    let candidate = output_dir.join(filesystem_path);
    // Normalize without requiring the file to exist yet for jail check on parents.
    let normalized = normalize_path(&candidate);
    if is_path_jail_escape(&output_dir, &normalized) {
        return Err(AppError::message("Not Found"));
    }
    if !normalized.exists() {
        // Directory → index.html
        if normalized.is_dir() || (!normalized.exists() && output_dir.join(filesystem_path).is_dir())
        {
            let index = normalized.join("index.html");
            if index.exists() && !is_path_jail_escape(&output_dir, &index) {
                return Ok(index);
            }
        }
        return Err(AppError::message(format!(
            "Not Found: {}",
            normalized.display()
        )));
    }
    if normalized.is_dir() {
        let index = normalized.join("index.html");
        if index.exists() {
            return Ok(index);
        }
        return Err(AppError::message("Not Found"));
    }
    Ok(normalized)
}

pub fn is_path_jail_escape(root: &Path, candidate: &Path) -> bool {
    !is_subpath(root, candidate)
        && !candidate.starts_with(root)
        && !path_is_under(root, candidate)
}

fn path_is_under(root: &Path, candidate: &Path) -> bool {
    let root_norm = normalize_path(root);
    let cand_norm = normalize_path(candidate);
    cand_norm.starts_with(&root_norm)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn file_server_bytes(file_path: &Path) -> Result<(String, Vec<u8>), AppError> {
    let path = if file_path.is_dir() {
        file_path.join("index.html")
    } else {
        file_path.to_path_buf()
    };
    if !path.exists() {
        return Err(AppError::message(format!("Not Found: {}", path.display())));
    }
    let bytes = std::fs::read(&path)?;
    Ok((content_type_for_path(&path).to_string(), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn content_types() {
        assert_eq!(content_type_for_path(Path::new("a.js")), "text/javascript");
        assert_eq!(content_type_for_path(Path::new("a.wasm")), "application/wasm");
        assert_eq!(content_type_for_path(Path::new("a.unknown")), "text/plain");
    }

    #[test]
    fn path_jail_blocks_traversal() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("bundle");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("ok.js"), b"1").unwrap();
        assert!(resolve_asset_file(&out, "ok.js").is_ok());
        assert!(resolve_asset_file(&out, "../secret").is_err());
        assert!(resolve_asset_file(&out, "foo/../../secret").is_err());
    }

    #[test]
    fn serves_index_html_for_directory() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("bundle");
        fs::create_dir_all(out.join("console")).unwrap();
        fs::write(out.join("console/index.html"), b"<html></html>").unwrap();
        let path = resolve_asset_file(&out, "console").unwrap();
        assert!(path.ends_with("index.html"));
    }
}
