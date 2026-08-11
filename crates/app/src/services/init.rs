//! App scaffolding: clone a GitHub template and render Liquid placeholders.

use crate::error::AppError;
use crate::utilities::liquid::recursive_liquid_template_copy;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: String,
    pub directory: PathBuf,
    /// Local template directory OR a GitHub URL (`https://github.com/org/repo` / `#branch`).
    pub template: String,
    pub package_manager: String,
    /// When true, `template` is treated as a local filesystem path (no git clone).
    pub local_template: bool,
}

#[derive(Debug, Clone)]
pub struct InitResult {
    pub output_directory: PathBuf,
}

/// Hyphenate an app name the way upstream `hyphenate` does for directory names.
pub fn hyphenate_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn init_app(options: InitOptions) -> Result<InitResult, AppError> {
    let hyphenized = hyphenate_name(&options.name);
    if hyphenized.is_empty() {
        return Err(AppError::message("App name cannot be empty"));
    }
    let output_directory = options.directory.join(&hyphenized);
    if output_directory.exists() {
        return Err(AppError::message(format!(
            "Directory {} already exists",
            output_directory.display()
        )));
    }

    let tmp = tempfile_dir()?;
    let download_dir = tmp.join("download");
    let scaffold_dir = tmp.join("app");
    fs::create_dir_all(&download_dir)?;

    let template_src = if options.local_template {
        PathBuf::from(&options.template)
    } else {
        clone_template(&options.template, &download_dir)?;
        resolve_template_subdir(&download_dir, &options.template)
    };

    if !template_src.is_dir() {
        return Err(AppError::message(format!(
            "Template path does not exist: {}",
            template_src.display()
        )));
    }

    recursive_liquid_template_copy(
        &template_src,
        &scaffold_dir,
        &json!({
            "dependency_manager": options.package_manager,
            "app_name": options.name,
        }),
    )
    .map_err(|e| AppError::message(e.to_string()))?;

    // Update package.json name when present.
    let pkg_path = scaffold_dir.join("package.json");
    if pkg_path.exists() {
        if let Ok(raw) = fs::read_to_string(&pkg_path) {
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("name".into(), json!(hyphenized));
                    obj.insert("private".into(), json!(true));
                }
                fs::write(
                    &pkg_path,
                    serde_json::to_string_pretty(&value).unwrap_or(raw),
                )?;
            }
        }
    }

    // Ensure a minimal shopify.app.toml exists for empty/local stubs.
    let app_toml = scaffold_dir.join("shopify.app.toml");
    if !app_toml.exists() {
        fs::write(
            &app_toml,
            format!(
                "name = \"{}\"\napplication_url = \"https://example.com\"\nembedded = true\n\n[access_scopes]\nscopes = \"write_products\"\n",
                options.name.replace('"', "\\\"")
            ),
        )?;
    }

    if let Some(parent) = output_directory.parent() {
        fs::create_dir_all(parent)?;
    }
    move_dir(&scaffold_dir, &output_directory)?;
    let _ = fs::remove_dir_all(&tmp);

    Ok(InitResult { output_directory })
}

fn tempfile_dir() -> Result<PathBuf, AppError> {
    let base = std::env::temp_dir().join(format!("shopify-app-init-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base)?;
    Ok(base)
}

/// Parse `https://github.com/org/repo` or `...#branch` or `.../tree/branch/path`.
pub fn parse_github_template_ref(template: &str) -> (String, Option<String>, Option<String>) {
    let (url_part, branch) = match template.split_once('#') {
        Some((u, b)) => (u, Some(b.to_string())),
        None => (template, None),
    };
    // Support tree/blob path forms loosely: keep base repo URL.
    let mut file_path = None;
    let mut clone_url = url_part.to_string();
    if let Some(idx) = url_part.find("/tree/") {
        let (base, rest) = url_part.split_at(idx);
        clone_url = base.to_string();
        let rest = rest.trim_start_matches("/tree/");
        let mut parts = rest.splitn(2, '/');
        let branch_from_tree = parts.next().map(str::to_string);
        file_path = parts.next().map(str::to_string);
        return (
            clone_url,
            branch.or(branch_from_tree),
            file_path,
        );
    }
    if !clone_url.ends_with(".git") && clone_url.contains("github.com") {
        clone_url.push_str(".git");
    }
    (clone_url, branch, file_path)
}

fn clone_template(template: &str, destination: &Path) -> Result<(), AppError> {
    let (url, branch, _) = parse_github_template_ref(template);
    let mut args = vec![
        "clone".to_string(),
        "--depth".into(),
        "1".into(),
        url.clone(),
        destination.display().to_string(),
    ];
    if let Some(branch) = branch {
        args.insert(1, "--branch".into());
        args.insert(2, branch);
    }
    let status = Command::new("git")
        .args(&args)
        .status()
        .map_err(|e| AppError::message(format!("Failed to run git: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "git clone failed for template {url}"
        )));
    }
    Ok(())
}

fn resolve_template_subdir(download_dir: &Path, template: &str) -> PathBuf {
    let (_, _, file_path) = parse_github_template_ref(template);
    match file_path {
        Some(sub) => download_dir.join(sub),
        None => download_dir.to_path_buf(),
    }
}

fn move_dir(from: &Path, to: &Path) -> Result<(), AppError> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Cross-device fallback.
    copy_dir_all(from, to)?;
    fs::remove_dir_all(from)?;
    Ok(())
}

fn copy_dir_all(from: &Path, to: &Path) -> Result<(), AppError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_all(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn hyphenates_names() {
        assert_eq!(hyphenate_name("My Cool App"), "my-cool-app");
        assert_eq!(hyphenate_name(" already-ok "), "already-ok");
    }

    #[test]
    fn parses_github_ref_with_branch() {
        let (url, branch, path) =
            parse_github_template_ref("https://github.com/Shopify/shopify-app-template-remix#main");
        assert!(url.contains("Shopify/shopify-app-template-remix"));
        assert_eq!(branch.as_deref(), Some("main"));
        assert!(path.is_none());
    }

    #[test]
    fn init_from_local_template_renders_liquid() {
        let dir = tempdir().unwrap();
        let template = dir.path().join("template");
        fs::create_dir_all(template.join("extensions")).unwrap();
        fs::write(
            template.join("README.md.liquid"),
            "# {{app_name}} with {{dependency_manager}}",
        )
        .unwrap();
        fs::write(
            template.join("package.json"),
            r#"{"name":"placeholder"}"#,
        )
        .unwrap();

        let parent = dir.path().join("out");
        fs::create_dir_all(&parent).unwrap();
        let result = init_app(InitOptions {
            name: "Demo App".into(),
            directory: parent.clone(),
            template: template.display().to_string(),
            package_manager: "npm".into(),
            local_template: true,
        })
        .unwrap();

        assert!(result.output_directory.ends_with("demo-app"));
        assert_eq!(
            fs::read_to_string(result.output_directory.join("README.md")).unwrap(),
            "# Demo App with npm"
        );
        let pkg: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(result.output_directory.join("package.json")).unwrap())
                .unwrap();
        assert_eq!(pkg["name"], "demo-app");
        assert!(result.output_directory.join("shopify.app.toml").exists());
    }

    #[test]
    fn refuses_existing_directory() {
        let dir = tempdir().unwrap();
        let existing = dir.path().join("demo-app");
        fs::create_dir_all(&existing).unwrap();
        let template = dir.path().join("template");
        fs::create_dir_all(&template).unwrap();
        let err = init_app(InitOptions {
            name: "Demo App".into(),
            directory: dir.path().to_path_buf(),
            template: template.display().to_string(),
            package_manager: "npm".into(),
            local_template: true,
        })
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
}
