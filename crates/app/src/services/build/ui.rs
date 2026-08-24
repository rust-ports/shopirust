use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bundle UI extension JS entry via `npx esbuild` with upstream-aligned flags.
pub fn build_ui_extension(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    build_ui_extension_with_options(ext, UiBundleOptions::production())
}

#[derive(Debug, Clone)]
pub struct UiBundleOptions {
    pub minify: bool,
    pub source_maps: bool,
    pub environment: &'static str,
    pub extra_defines: Vec<(String, String)>,
}

impl UiBundleOptions {
    pub fn production() -> Self {
        Self {
            minify: true,
            source_maps: false,
            environment: "production",
            extra_defines: vec![],
        }
    }

    pub fn development() -> Self {
        let minify = std::env::var("SHOPIFY_CLI_DISABLE_MINIFICATION_ON_DEV").is_err();
        Self {
            minify,
            source_maps: true,
            environment: "development",
            extra_defines: vec![],
        }
    }
}

pub fn build_ui_extension_with_options(
    ext: &ExtensionInstance,
    options: UiBundleOptions,
) -> Result<PathBuf, AppError> {
    let dist = ext.directory.join("dist");
    fs::create_dir_all(&dist)?;

    let entries = collect_entries(ext);
    if entries.is_empty() {
        return Err(AppError::message(format!(
            "UI extension '{}' has no JS entrypoint (src/index.{{js,jsx,ts,tsx}})",
            ext.handle
        )));
    }

    let outfile = dist.join("index.js");
    let mut args = vec![
        "--yes".into(),
        "esbuild".into(),
        entries[0].to_string_lossy().into_owned(),
        "--bundle".into(),
        "--outfile".into(),
        outfile.to_string_lossy().into_owned(),
        "--format=esm".into(),
        "--target=es6".into(),
        "--jsx=automatic".into(),
        "--loader:.js=jsx".into(),
        "--loader:.esnext=ts".into(),
        format!("--define:process.env.NODE_ENV=\"{}\"", options.environment),
    ];
    if options.minify {
        args.push("--minify".into());
    }
    if options.source_maps {
        args.push("--sourcemap".into());
        args.push(format!(
            "--source-root={}",
            ext.directory.join("src").display()
        ));
    }
    for (k, v) in &options.extra_defines {
        args.push(format!("--define:process.env.{k}=\"{v}\""));
    }
    // Match graphql-loader: allow importing .graphql files as text.
    args.push("--loader:.graphql=text".into());
    args.push("--log-level=error".into());

    let status = Command::new("npx")
        .args(&args)
        .current_dir(&ext.directory)
        .status();

    match status {
        Ok(s) if s.success() => {
            for extra in entries.iter().skip(1) {
                // should-render / additional targets: bundle beside index as handle-N.js
                let extra_out = dist.join(format!(
                    "{}-{}.js",
                    ext.handle,
                    extra
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("extra")
                ));
                let _ = Command::new("npx")
                    .args([
                        "--yes",
                        "esbuild",
                        &extra.to_string_lossy(),
                        "--bundle",
                        "--outfile",
                        &extra_out.to_string_lossy(),
                        "--format=esm",
                        "--target=es6",
                    ])
                    .current_dir(&ext.directory)
                    .status();
            }
            Ok(outfile)
        }
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

fn collect_entries(ext: &ExtensionInstance) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    if let Some(arr) = ext
        .configuration
        .get("extension_points")
        .or_else(|| ext.configuration.get("targeting"))
        .and_then(|v| v.as_array())
    {
        for ep in arr {
            if let Some(m) = ep.get("module").and_then(|m| m.as_str()) {
                let path = ext.directory.join(m);
                if path.exists() {
                    entries.push(path);
                }
            }
        }
    }
    if entries.is_empty() {
        if let Some(p) = find_default_entry(&ext.directory) {
            entries.push(p);
        }
    }
    entries
}

fn find_default_entry(dir: &Path) -> Option<PathBuf> {
    for name in [
        "src/index.ts",
        "src/index.tsx",
        "src/index.js",
        "src/index.jsx",
        "index.ts",
        "index.js",
    ] {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn finds_src_index_ts() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/index.ts"), "export {}").unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let ext = ExtensionInstance::new(
            "ui",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let entries = collect_entries(&ext);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ends_with("src/index.ts"));
    }

    #[test]
    fn targeting_module_preferred() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/checkout.ts"), "export {}").unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert(
            "targeting".into(),
            serde_json::json!([{ "target": "purchase.checkout.block.render", "module": "./src/checkout.ts" }]),
        );
        let ext = ExtensionInstance::new(
            "ui",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            cfg,
            spec,
        );
        let entries = collect_entries(&ext);
        assert!(entries[0].ends_with("checkout.ts"));
    }

    #[test]
    fn missing_entry_errors() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let ext = ExtensionInstance::new(
            "ui",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let err = build_ui_extension(&ext).unwrap_err();
        assert!(err.to_string().contains("no JS entrypoint"));
    }
}
