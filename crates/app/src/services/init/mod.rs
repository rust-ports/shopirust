//! App scaffolding: clone a GitHub template and render Liquid placeholders.

pub mod cleanup;
pub mod npm;
pub mod templates;
pub mod validate;

use crate::error::AppError;
use crate::services::dependencies::install_app_dependencies;
use crate::utilities::liquid::recursive_liquid_template_copy;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use templates::resolve_template_url;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: String,
    pub directory: PathBuf,
    /// Local template directory OR a GitHub URL (`https://github.com/org/repo` / `#branch`).
    pub template: String,
    pub package_manager: String,
    /// When true, `template` is treated as a local filesystem path (no git clone).
    pub local_template: bool,
    pub flavor: Option<String>,
    pub client_id: Option<String>,
    pub organization_id: Option<String>,
    /// Install npm/yarn/pnpm after scaffold (upstream default).
    pub install_dependencies: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            name: String::new(),
            directory: PathBuf::from("."),
            template: String::new(),
            package_manager: "npm".into(),
            local_template: false,
            flavor: None,
            client_id: None,
            organization_id: None,
            install_dependencies: false,
        }
    }
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

    let resolved_template = resolve_template_url(&options.template, options.flavor.as_deref());

    let tmp = tempfile_dir()?;
    let download_dir = tmp.join("download");
    let scaffold_dir = tmp.join("app");
    fs::create_dir_all(&download_dir)?;

    let template_src = if options.local_template {
        PathBuf::from(&options.template)
    } else {
        clone_template(&resolved_template, &download_dir)?;
        resolve_template_subdir(&download_dir, &resolved_template)
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

    let app_toml = scaffold_dir.join("shopify.app.toml");
    if !app_toml.exists() {
        let mut toml = format!(
            "name = \"{}\"\napplication_url = \"https://example.com\"\nembedded = true\n\n[access_scopes]\nscopes = \"write_products\"\n",
            options.name.replace('"', "\\\"")
        );
        if let Some(ref client_id) = options.client_id {
            toml = format!("client_id = \"{client_id}\"\n{toml}");
        }
        fs::write(&app_toml, toml)?;
    } else if let Some(ref client_id) = options.client_id {
        let raw = fs::read_to_string(&app_toml)?;
        if !raw.contains("client_id") {
            fs::write(&app_toml, format!("client_id = \"{client_id}\"\n{raw}"))?;
        }
    }
    let _ = options.organization_id;

    if let Some(parent) = output_directory.parent() {
        fs::create_dir_all(parent)?;
    }
    move_dir(&scaffold_dir, &output_directory)?;
    let _ = fs::remove_dir_all(&tmp);

    if options.install_dependencies {
        let _ = install_app_dependencies(&output_directory, false, None);
    }

    Ok(InitResult { output_directory })
}

fn tempfile_dir() -> Result<PathBuf, AppError> {
    let base = std::env::temp_dir().join(format!(
        "shopify-app-init-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&base)?;
    Ok(base)
}

/// Parse `https://github.com/org/repo` or `...#branch` or `.../tree/branch/path`.
pub fn parse_github_template_ref(template: &str) -> (String, Option<String>, Option<String>) {
    let (url_part, branch) = match template.split_once('#') {
        Some((u, b)) => (u, Some(b.to_string())),
        None => (template, None),
    };
    let mut file_path = None;
    let mut clone_url = url_part.to_string();
    if let Some(idx) = url_part.find("/tree/") {
        let (base, rest) = url_part.split_at(idx);
        clone_url = base.to_string();
        let rest = rest.trim_start_matches("/tree/");
        let mut parts = rest.splitn(2, '/');
        let branch_from_tree = parts.next().map(str::to_string);
        file_path = parts.next().map(str::to_string);
        return (clone_url, branch.or(branch_from_tree), file_path);
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

    fn opts(name: &str, directory: PathBuf, template: String) -> InitOptions {
        InitOptions {
            name: name.into(),
            directory,
            template,
            package_manager: "npm".into(),
            local_template: true,
            ..Default::default()
        }
    }

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
        fs::write(template.join("package.json"), r#"{"name":"placeholder"}"#).unwrap();

        let parent = dir.path().join("out");
        fs::create_dir_all(&parent).unwrap();
        let result = init_app(opts("Demo App", parent, template.display().to_string())).unwrap();

        assert!(result.output_directory.ends_with("demo-app"));
        assert_eq!(
            fs::read_to_string(result.output_directory.join("README.md")).unwrap(),
            "# Demo App with npm"
        );
        let pkg: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(result.output_directory.join("package.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(pkg["name"], "demo-app");
        assert!(result.output_directory.join("shopify.app.toml").exists());
    }

    #[test]
    fn writes_client_id_when_provided() {
        let dir = tempdir().unwrap();
        let template = dir.path().join("template");
        fs::create_dir_all(&template).unwrap();
        let mut options = opts(
            "With Client",
            dir.path().to_path_buf(),
            template.display().to_string(),
        );
        options.client_id = Some("gid://app/1".into());
        let result = init_app(options).unwrap();
        let toml = fs::read_to_string(result.output_directory.join("shopify.app.toml")).unwrap();
        assert!(toml.contains("gid://app/1"));
    }

    #[test]
    fn empty_name_is_rejected() {
        let dir = tempdir().unwrap();
        let err = init_app(opts(
            "   ",
            dir.path().to_path_buf(),
            dir.path().join("template").display().to_string(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn flavor_is_applied_to_template_url() {
        let url = crate::services::init::templates::resolve_template_url(
            "reactRouter",
            Some("typescript"),
        );
        assert!(url.ends_with("#main-cli"));
    }

    #[test]
    fn missing_local_template_errors() {
        let dir = tempdir().unwrap();
        let err = init_app(opts(
            "Demo",
            dir.path().to_path_buf(),
            dir.path().join("no-such-template").display().to_string(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn refuses_existing_directory() {
        let dir = tempdir().unwrap();
        let existing = dir.path().join("demo-app");
        fs::create_dir_all(&existing).unwrap();
        let template = dir.path().join("template");
        fs::create_dir_all(&template).unwrap();
        let err = init_app(opts(
            "Demo App",
            dir.path().to_path_buf(),
            template.display().to_string(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
}
