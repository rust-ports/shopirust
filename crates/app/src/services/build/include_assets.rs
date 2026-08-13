//! Include-assets build step (upstream `include-assets-step.ts`).

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InclusionEntry {
    Pattern {
        #[serde(default, rename = "baseDir")]
        base_dir: Option<String>,
        #[serde(default = "default_include")]
        include: Vec<String>,
        #[serde(default)]
        ignore: Vec<String>,
        destination: Option<String>,
    },
    Static {
        source: String,
        destination: Option<String>,
    },
    #[serde(rename = "configKey")]
    ConfigKey {
        key: String,
        destination: Option<String>,
        #[serde(default)]
        preserve_file_paths: bool,
    },
}

fn default_include() -> Vec<String> {
    vec!["**/*".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IncludeAssetsConfig {
    #[serde(default)]
    pub inclusions: Vec<InclusionEntry>,
    #[serde(default)]
    pub generates_assets_manifest: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IncludeAssetsResult {
    pub files_copied: usize,
    pub output_paths: Vec<PathBuf>,
}

/// Ensure `resolved` is inside `app_directory` (rejects `..` path segments).
pub fn assert_path_within_app_dir(
    resolved: &Path,
    app_directory: &Path,
    config_value: &str,
) -> Result<(), AppError> {
    let app = app_directory
        .canonicalize()
        .unwrap_or_else(|_| app_directory.to_path_buf());
    let src = resolved
        .canonicalize()
        .unwrap_or_else(|_| resolved.to_path_buf());
    let rel = pathdiff_simple(&app, &src);
    let first = rel.split(['/', '\\']).next().unwrap_or("");
    if first == ".." || Path::new(&rel).is_absolute() {
        return Err(AppError::message(format!(
            "Asset path '{config_value}' resolves outside the app directory. Resolved to: {}",
            src.display()
        )));
    }
    Ok(())
}

fn pathdiff_simple(base: &Path, target: &Path) -> String {
    pathdiff(base, target).unwrap_or_else(|| target.display().to_string())
}

fn pathdiff(base: &Path, target: &Path) -> Option<String> {
    let base_c = base.components().collect::<Vec<_>>();
    let target_c = target.components().collect::<Vec<_>>();
    let mut i = 0;
    while i < base_c.len() && i < target_c.len() && base_c[i] == target_c[i] {
        i += 1;
    }
    let mut out = PathBuf::new();
    for _ in i..base_c.len() {
        out.push("..");
    }
    for c in &target_c[i..] {
        out.push(c.as_os_str());
    }
    Some(out.to_string_lossy().to_string())
}

/// Copy declared assets. Supports the upstream inclusion matrix plus the legacy `assets` array.
pub fn include_assets_step(ext: &ExtensionInstance) -> Result<Vec<PathBuf>, AppError> {
    execute_include_assets(
        ext,
        ext.output_path
            .clone()
            .unwrap_or_else(|| ext.directory.join("dist")),
        &ext.directory,
    )
    .map(|r| r.output_paths)
}

pub fn execute_include_assets(
    ext: &ExtensionInstance,
    output_dir: PathBuf,
    app_directory: &Path,
) -> Result<IncludeAssetsResult, AppError> {
    if let Some(config) = parse_step_config(&ext.configuration) {
        return run_inclusions(ext, &config, output_dir, app_directory);
    }
    legacy_assets_array(ext, output_dir, app_directory)
}

fn parse_step_config(configuration: &std::collections::HashMap<String, Value>) -> Option<IncludeAssetsConfig> {
    if let Some(inclusions) = configuration.get("inclusions") {
        return serde_json::from_value(serde_json::json!({
            "inclusions": inclusions,
            "generatesAssetsManifest": configuration.get("generates_assets_manifest").cloned().unwrap_or(Value::Bool(false)),
        }))
        .ok();
    }
    configuration
        .get("include_assets")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn legacy_assets_array(
    ext: &ExtensionInstance,
    output_dir: PathBuf,
    app_directory: &Path,
) -> Result<IncludeAssetsResult, AppError> {
    let assets = ext
        .configuration
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if assets.is_empty() {
        return Ok(IncludeAssetsResult::default());
    }
    fs::create_dir_all(&output_dir)?;
    let mut result = IncludeAssetsResult::default();
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
        assert_path_within_app_dir(&src, app_directory, rel)?;
        let dest = output_dir.join(src.file_name().unwrap_or_default());
        fs::copy(&src, &dest)?;
        result.files_copied += 1;
        result.output_paths.push(dest);
    }
    Ok(result)
}

fn run_inclusions(
    ext: &ExtensionInstance,
    config: &IncludeAssetsConfig,
    output_dir: PathBuf,
    app_directory: &Path,
) -> Result<IncludeAssetsResult, AppError> {
    fs::create_dir_all(&output_dir)?;
    let mut result = IncludeAssetsResult::default();
    let mut manifest: serde_json::Map<String, Value> = serde_json::Map::new();

    for entry in &config.inclusions {
        match entry {
            InclusionEntry::ConfigKey {
                key,
                destination,
                preserve_file_paths,
            } => {
                let _ = preserve_file_paths;
                let paths = lookup_flattened_paths(&ext.configuration, key);
                if paths.is_empty() && lookup_config_key(&ext.configuration, key).is_none() && !key.contains("[]") {
                    continue;
                }
                for rel in paths {
                    let src = ext.directory.join(&rel);
                    if !src.exists() {
                        return Err(AppError::message(format!(
                            "Referenced file does not exist: {}",
                            src.display()
                        )));
                    }
                    assert_path_within_app_dir(&src, app_directory, &rel)?;
                    let dest = if let Some(d) = destination {
                        let dest = output_dir.join(d).join(src.file_name().unwrap_or_default());
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        copy_path(&src, &dest)?;
                        dest
                    } else {
                        copy_into(&src, &output_dir)?
                    };
                    result.files_copied += count_files(&dest);
                    result.output_paths.push(dest);
                }
            }
            InclusionEntry::Static {
                source,
                destination,
            } => {
                let src = ext.directory.join(source);
                if !src.exists() {
                    return Err(AppError::message(format!(
                        "Source does not exist: {}",
                        src.display()
                    )));
                }
                assert_path_within_app_dir(&src, app_directory, source)?;
                let dest = match destination {
                    Some(d) if !d.is_empty() => output_dir.join(d),
                    _ => output_dir.join(src.file_name().unwrap_or_default()),
                };
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                copy_path(&src, &dest)?;
                result.files_copied += count_files(&dest);
                result.output_paths.push(dest);
            }
            InclusionEntry::Pattern {
                base_dir,
                include,
                ignore,
                destination,
            } => {
                let source_dir = match base_dir {
                    Some(b) => ext.directory.join(b),
                    None => ext.directory.clone(),
                };
                if !source_dir.exists() {
                    continue;
                }
                let config_value = base_dir.as_deref().unwrap_or(".");
                assert_path_within_app_dir(&source_dir, app_directory, config_value)?;
                let dest_dir = match destination {
                    Some(d) if !d.is_empty() => output_dir.join(d),
                    _ => output_dir.clone(),
                };
                fs::create_dir_all(&dest_dir)?;
                let files = glob_files(&source_dir, include, ignore);
                for file in files {
                    let rel = file.strip_prefix(&source_dir).unwrap_or(&file);
                    let dest = dest_dir.join(rel);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    if file != dest {
                        fs::copy(&file, &dest)?;
                        result.files_copied += 1;
                        result.output_paths.push(dest);
                    }
                }
            }
        }
    }

    if config.generates_assets_manifest {
        for path in &result.output_paths {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                manifest.insert(
                    name.to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
        }
        fs::write(
            output_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".into()),
        )?;
    }
    Ok(result)
}

fn lookup_config_key<'a>(
    configuration: &'a std::collections::HashMap<String, Value>,
    key: &str,
) -> Option<&'a Value> {
    if key.contains("[]") {
        return None;
    }
    let mut parts = key.split('.');
    let first = parts.next()?;
    let mut v = configuration.get(first)?;
    for part in parts {
        v = v.get(part)?;
    }
    Some(v)
}

/// Collect file paths from a config value, including `key[].field` flatten.
fn lookup_flattened_paths(
    configuration: &std::collections::HashMap<String, Value>,
    key: &str,
) -> Vec<String> {
    if let Some(idx) = key.find("[]") {
        let before = &key[..idx];
        let after = key[idx + 2..].trim_start_matches('.');
        let Some(arr) = lookup_config_key(configuration, before).and_then(|v| v.as_array()) else {
            return vec![];
        };
        return arr
            .iter()
            .filter_map(|item| {
                if after.is_empty() {
                    item.as_str().map(str::to_string)
                } else {
                    item.pointer(&format!("/{}", after.replace('.', "/")))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                }
            })
            .collect();
    }
    lookup_config_key(configuration, key)
        .map(json_paths)
        .unwrap_or_default()
}

fn json_paths(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(arr) => arr
            .iter()
            .flat_map(json_paths)
            .collect(),
        Value::Object(map) => map.values().flat_map(json_paths).collect(),
        _ => vec![],
    }
}

fn copy_path(src: &Path, dest: &Path) -> Result<(), AppError> {
    if src.is_dir() {
        copy_dir(src, dest)?;
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dest)?;
    }
    Ok(())
}

fn copy_into(src: &Path, dest_dir: &Path) -> Result<PathBuf, AppError> {
    let dest = dest_dir.join(src.file_name().unwrap_or_default());
    copy_path(src, &dest)?;
    Ok(dest)
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), AppError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn count_files(path: &Path) -> usize {
    if path.is_file() {
        return 1;
    }
    let mut n = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            n += count_files(&entry.path());
        }
    }
    n
}

fn glob_files(source_dir: &Path, include: &[String], ignore: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(source_dir, source_dir, include, ignore, &mut files);
    files
}

fn walk(
    root: &Path,
    dir: &Path,
    include: &[String],
    ignore: &[String],
    out: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if ignore.iter().any(|g| glob_match(g, &rel)) {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, include, ignore, out);
        } else if include.iter().any(|g| glob_match(g, &rel)) {
            out.push(path);
        }
    }
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**/*" || pattern == "*" {
        return true;
    }
    if let Ok(pat) = glob::Pattern::new(pattern) {
        if pat.matches(path) {
            return true;
        }
        if let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) {
            return pat.matches(name);
        }
    }
    path.ends_with(pattern.trim_start_matches("**/")) || path == pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn ext_at(dir: &Path, configuration: HashMap<String, Value>) -> ExtensionInstance {
        ExtensionInstance::new(
            "x",
            dir.to_path_buf(),
            dir.join("shopify.extension.toml"),
            configuration,
            create_extension_specification("theme").unwrap(),
        )
    }

    #[test]
    fn copies_listed_assets() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("logo.png"), b"png").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert("assets".into(), serde_json::json!(["logo.png"]));
        let ext = ext_at(dir.path(), configuration);
        let copied = include_assets_step(&ext).unwrap();
        assert_eq!(copied.len(), 1);
        assert!(dir.path().join("dist/logo.png").exists());
    }

    #[test]
    fn static_copies_directory_under_own_name() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("dist")).unwrap();
        fs::write(dir.path().join("dist/index.html"), "hi").unwrap();
        fs::create_dir_all(dir.path().join("dist/assets")).unwrap();
        fs::write(dir.path().join("dist/assets/logo.png"), "png").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "dist" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        let result = execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("dist/index.html").exists());
        assert!(out.join("dist/assets/logo.png").exists());
        assert!(result.files_copied >= 2);
    }

    #[test]
    fn static_missing_source_errors() {
        let dir = tempdir().unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "missing" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let err = execute_include_assets(&ext, dir.path().join("out"), dir.path()).unwrap_err();
        assert!(err.to_string().contains("Source does not exist"));
    }

    #[test]
    fn static_file_to_destination() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("logo.png"), b"png").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "logo.png", "destination": "assets/logo.png" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("assets/logo.png").exists());
    }

    #[test]
    fn pattern_copies_matching_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("keep.js"), "k").unwrap();
        fs::write(dir.path().join("skip.txt"), "s").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "pattern", "include": ["*.js"] }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("keep.js").exists());
        assert!(!out.join("skip.txt").exists());
    }

    #[test]
    fn pattern_respects_ignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.js"), "a").unwrap();
        fs::write(dir.path().join("b.js"), "b").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "pattern", "include": ["*.js"], "ignore": ["b.js"] }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("a.js").exists());
        assert!(!out.join("b.js").exists());
    }

    #[test]
    fn config_key_copies_and_skips_absent() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("icon.svg"), "<svg/>").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert("icon".into(), serde_json::json!("icon.svg"));
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([
                { "type": "configKey", "key": "icon" },
                { "type": "configKey", "key": "missing" }
            ]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        let result = execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("icon.svg").exists());
        assert_eq!(result.files_copied, 1);
    }

    #[test]
    fn rejects_path_outside_app() {
        let dir = tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "x").unwrap();
        // symlink or `..` relative
        let err = assert_path_within_app_dir(
            &outside.join("secret.txt"),
            &dir.path().join("app"),
            "../outside/secret.txt",
        );
        assert!(err.is_err());
    }

    #[test]
    fn writes_manifest_when_requested() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "a.txt" }]),
        );
        configuration.insert("generates_assets_manifest".into(), serde_json::json!(true));
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("manifest.json").exists());
    }

    #[test]
    fn does_not_write_manifest_by_default() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "a.txt" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(!out.join("manifest.json").exists());
    }

    #[test]
    fn multiple_static_entries() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([
                { "type": "static", "source": "a.txt" },
                { "type": "static", "source": "b.txt" }
            ]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        let result = execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("a.txt").exists());
        assert!(out.join("b.txt").exists());
        assert!(result.files_copied >= 2);
    }

    #[test]
    fn pattern_copies_to_destination_subdir() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("keep.js"), "k").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "pattern", "include": ["*.js"], "destination": "assets" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("assets/keep.js").exists());
    }

    #[test]
    fn config_key_missing_file_errors() {
        let dir = tempdir().unwrap();
        let mut configuration = HashMap::new();
        configuration.insert("icon".into(), serde_json::json!("missing.svg"));
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "configKey", "key": "icon" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let err = execute_include_assets(&ext, dir.path().join("out"), dir.path()).unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn config_key_array_copies_each_path() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.svg"), "a").unwrap();
        fs::write(dir.path().join("b.svg"), "b").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert("icons".into(), serde_json::json!(["a.svg", "b.svg"]));
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "configKey", "key": "icons" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("a.svg").exists());
        assert!(out.join("b.svg").exists());
    }

    #[test]
    fn config_key_flatten_collects_leaf_paths() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("one.js"), "1").unwrap();
        fs::write(dir.path().join("two.js"), "2").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "targeting".into(),
            serde_json::json!([
                { "module": "one.js" },
                { "module": "two.js" }
            ]),
        );
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "configKey", "key": "targeting[].module" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("one.js").exists());
        assert!(out.join("two.js").exists());
    }

    #[test]
    fn mixed_static_and_pattern() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("logo.png"), "p").unwrap();
        fs::write(dir.path().join("keep.js"), "k").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([
                { "type": "static", "source": "logo.png" },
                { "type": "pattern", "include": ["*.js"] }
            ]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("logo.png").exists());
        assert!(out.join("keep.js").exists());
    }

    #[test]
    fn pattern_zero_matches_is_ok() {
        let dir = tempdir().unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "pattern", "include": ["*.wasm"] }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let result = execute_include_assets(&ext, dir.path().join("out"), dir.path()).unwrap();
        assert_eq!(result.files_copied, 0);
    }

    #[test]
    fn static_directory_to_explicit_destination() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/index.html"), "hi").unwrap();
        let mut configuration = HashMap::new();
        configuration.insert(
            "inclusions".into(),
            serde_json::json!([{ "type": "static", "source": "src", "destination": "public" }]),
        );
        let ext = ext_at(dir.path(), configuration);
        let out = dir.path().join("output");
        execute_include_assets(&ext, out.clone(), dir.path()).unwrap();
        assert!(out.join("public/index.html").exists());
    }
}
