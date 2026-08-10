use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use std::fs;
use std::path::PathBuf;

/// Copy declared asset files into the extension dist folder.
pub fn include_assets_step(ext: &ExtensionInstance) -> Result<Vec<PathBuf>, AppError> {
    let assets = ext
        .configuration
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if assets.is_empty() {
        return Ok(vec![]);
    }

    let dist = ext.directory.join("dist");
    fs::create_dir_all(&dist)?;
    let mut copied = Vec::new();
    for asset in assets {
        let rel = asset
            .as_str()
            .or_else(|| asset.get("filepath").and_then(|v| v.as_str()))
            .unwrap_or("");
        if rel.is_empty() {
            continue;
        }
        let src = ext.directory.join(rel);
        if !src.exists() {
            continue;
        }
        let dest = dist.join(src.file_name().unwrap_or_default());
        fs::copy(&src, &dest)?;
        copied.push(dest);
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn copies_listed_assets() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("logo.png"), b"png").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "assets".into(),
            serde_json::json!(["logo.png"]),
        );
        let ext = ExtensionInstance::new(
            "x",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            configuration,
            create_extension_specification("theme").unwrap(),
        );
        let copied = include_assets_step(&ext).unwrap();
        assert_eq!(copied.len(), 1);
        assert!(dir.path().join("dist/logo.png").exists());
    }
}
