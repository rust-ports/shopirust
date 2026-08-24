use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use std::fs;
use std::path::PathBuf;

/// Theme extensions: copy liquid/blocks/assets into `dist/`.
pub fn build_theme_extension(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    let dist = ext.directory.join("dist");
    if dist.exists() {
        fs::remove_dir_all(&dist)?;
    }
    fs::create_dir_all(&dist)?;

    for name in ["blocks", "snippets", "assets", "locales"] {
        let src = ext.directory.join(name);
        if src.is_dir() {
            copy_dir(&src, &dist.join(name))?;
        }
    }
    // Also copy root liquid files if any
    for entry in fs::read_dir(&ext.directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extn) = path.extension().and_then(|e| e.to_str()) {
                if matches!(extn, "liquid" | "json") {
                    fs::copy(&path, dist.join(entry.file_name()))?;
                }
            }
        }
    }
    Ok(dist)
}

fn copy_dir(src: &std::path::Path, dest: &std::path::Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn copies_blocks_and_root_liquid() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("blocks")).unwrap();
        std::fs::write(
            dir.path().join("blocks/app.liquid"),
            "{% comment %}x{% endcomment %}",
        )
        .unwrap();
        std::fs::write(dir.path().join("settings_data.json"), "{}").unwrap();
        let spec = create_extension_specification("theme").unwrap();
        let ext = ExtensionInstance::new(
            "theme-ext",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let dist = build_theme_extension(&ext).unwrap();
        assert!(dist.join("blocks/app.liquid").exists());
        assert!(dist.join("settings_data.json").exists());
    }
}
