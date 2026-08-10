use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Build or copy a function wasm artifact into `dist/`.
pub fn build_function_extension(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    let dist = ext.directory.join("dist");
    fs::create_dir_all(&dist)?;

    // Prefer existing wasm
    for candidate in [
        ext.directory.join("dist/index.wasm"),
        ext.directory.join("index.wasm"),
        ext.directory
            .join("target/wasm32-wasip1/release")
            .join(format!("{}.wasm", ext.handle.replace('-', "_"))),
    ] {
        if candidate.exists() {
            let dest = dist.join("index.wasm");
            if candidate != dest {
                fs::copy(&candidate, &dest)?;
            }
            return Ok(dest);
        }
    }

    // Try cargo build if Cargo.toml present
    if ext.directory.join("Cargo.toml").exists() {
        let status = Command::new("cargo")
            .args(["build", "--release", "--target", "wasm32-wasip1"])
            .current_dir(&ext.directory)
            .status();
        if let Ok(s) = status {
            if s.success() {
                // search for wasm under target
                if let Ok(entries) =
                    fs::read_dir(ext.directory.join("target/wasm32-wasip1/release"))
                {
                    for entry in entries.flatten() {
                        if entry.path().extension().and_then(|e| e.to_str()) == Some("wasm") {
                            let dest = dist.join("index.wasm");
                            fs::copy(entry.path(), &dest)?;
                            return Ok(dest);
                        }
                    }
                }
            }
        }
    }

    Err(AppError::message(format!(
        "Function extension '{}' has no wasm artifact. Build it first or place index.wasm in dist/.",
        ext.handle
    )))
}
