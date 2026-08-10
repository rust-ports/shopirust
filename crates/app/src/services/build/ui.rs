use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Bundle UI extension JS entry via `npx esbuild` when available.
pub fn build_ui_extension(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    let dist = ext.directory.join("dist");
    fs::create_dir_all(&dist)?;

    let entry = find_entry(ext).ok_or_else(|| {
        AppError::message(format!(
            "UI extension '{}' has no JS entrypoint (src/index.{{js,jsx,ts,tsx}})",
            ext.handle
        ))
    })?;

    let outfile = dist.join("index.js");
    let status = Command::new("npx")
        .args([
            "--yes",
            "esbuild",
            &entry.to_string_lossy(),
            "--bundle",
            "--outfile",
            &outfile.to_string_lossy(),
            "--format=esm",
            "--target=es2020",
        ])
        .current_dir(&ext.directory)
        .status();

    match status {
        Ok(s) if s.success() => Ok(outfile),
        Ok(_) => Err(AppError::message(format!(
            "esbuild failed for '{}'. Ensure Node.js is installed and the entry compiles.",
            ext.handle
        ))),
        Err(_) => Err(AppError::message(format!(
            "UI extension '{}' requires esbuild via npx. Install Node.js or pass --no-build on deploy.",
            ext.handle
        ))),
    }
}

fn find_entry(ext: &ExtensionInstance) -> Option<PathBuf> {
    if let Some(p) = ext
        .configuration
        .get("extension_points")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|ep| ep.get("module"))
        .and_then(|m| m.as_str())
    {
        let path = ext.directory.join(p);
        if path.exists() {
            return Some(path);
        }
    }
    for name in [
        "src/index.ts",
        "src/index.tsx",
        "src/index.js",
        "src/index.jsx",
        "index.ts",
        "index.js",
    ] {
        let path = ext.directory.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}
