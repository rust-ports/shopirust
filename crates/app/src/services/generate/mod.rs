//! Generate extension scaffolds from Liquid templates.

use crate::error::AppError;
use crate::models::loader::LoadedApp;
use crate::utilities::liquid::recursive_liquid_template_copy;
use cli_api::{
    DeveloperPlatformClient, ExtensionTemplate, ExtensionTemplatesResult, MinimalAppIdentifiers,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GenerateExtensionOptions {
    pub name: String,
    pub extension_type: String,
    pub flavor: Option<String>,
    /// Local template directory or GitHub URL.
    pub template: String,
    pub local_template: bool,
    /// Optional clone URL override (upstream `cloneUrl`).
    pub clone_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GeneratedExtension {
    pub directory: PathBuf,
    pub handle: String,
    pub extension_type: String,
}

/// Fetch remote extension templates and filter by local specification identifiers.
pub async fn fetch_extension_templates(
    client: &dyn DeveloperPlatformClient,
    app: &MinimalAppIdentifiers,
    available_specifications: &[String],
) -> Result<ExtensionTemplatesResult, AppError> {
    let result = client
        .template_specifications(app)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let templates: Vec<ExtensionTemplate> = result
        .templates
        .into_iter()
        .filter(|t| {
            available_specifications.iter().any(|id| {
                id == &t.identifier
                    || t.types.iter().any(|ty| {
                        ty.get("type")
                            .and_then(|v| v.as_str())
                            .is_some_and(|s| s == id)
                    })
            })
        })
        .collect();
    Ok(ExtensionTemplatesResult { templates })
}

/// Resolve a flavor subdirectory inside a cloned template (or local tree).
pub fn resolve_flavor_directory(root: &Path, flavor: Option<&str>) -> PathBuf {
    resolve_flavor_subdir(root, flavor).unwrap_or_else(|_| root.to_path_buf())
}

/// Slugify for extension handles / directory names.
pub fn slugify(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Deterministic UUID-like id from a handle (upstream `nonRandomUUID(slugify(name))` approx).
pub fn deterministic_uid(handle: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    handle.hash(&mut hasher);
    let h = hasher.finish();
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (h >> 32) as u32,
        ((h >> 16) & 0xffff) as u16,
        ((h >> 4) & 0xfff) as u16,
        (h & 0xfff) as u16,
        h
    )
}

pub fn ensure_extension_directory(app: &LoadedApp, name: &str) -> Result<PathBuf, AppError> {
    let handle = slugify(name);
    if handle.is_empty() {
        return Err(AppError::message("Extension name cannot be empty"));
    }
    let directory = app.directory.join("extensions").join(&handle);
    if directory.exists() {
        return Err(AppError::message(format!(
            "Extension directory already exists: {}",
            directory.display()
        )));
    }
    fs::create_dir_all(&directory)?;
    // Lock file while scaffolding (removed on success).
    fs::write(directory.join(".shopify.lock"), "")?;
    Ok(directory)
}

pub fn generate_extension(
    app: &LoadedApp,
    options: GenerateExtensionOptions,
) -> Result<GeneratedExtension, AppError> {
    let handle = slugify(&options.name);
    let directory = ensure_extension_directory(app, &options.name)?;

    let result = (|| -> Result<GeneratedExtension, AppError> {
        let tmp =
            std::env::temp_dir().join(format!("shopify-ext-gen-{}-{}", std::process::id(), handle));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;

        let template_url = options
            .clone_url
            .as_deref()
            .unwrap_or(options.template.as_str());
        let template_src = if options.local_template {
            resolve_flavor_directory(&PathBuf::from(template_url), options.flavor.as_deref())
        } else {
            let download = tmp.join("download");
            clone_shallow(template_url, &download)?;
            resolve_flavor_subdir(&download, options.flavor.as_deref())?
        };

        let data = json!({
            "name": options.name,
            "type": options.extension_type,
            "uid": deterministic_uid(&handle),
            "handle": handle,
            "flavor": options.flavor,
        });

        recursive_liquid_template_copy(&template_src, &directory, &data)
            .map_err(|e| AppError::message(e.to_string()))?;

        // Ensure a toml exists for minimal stubs.
        ensure_extension_toml(&directory, &options.extension_type, &handle, &options.name)?;

        let _ = fs::remove_file(directory.join(".shopify.lock"));
        let _ = fs::remove_dir_all(&tmp);

        Ok(GeneratedExtension {
            directory: PathBuf::from("extensions").join(&handle),
            handle,
            extension_type: options.extension_type.clone(),
        })
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&directory);
    }
    result
}

fn ensure_extension_toml(
    directory: &Path,
    extension_type: &str,
    handle: &str,
    name: &str,
) -> Result<(), AppError> {
    let toml_path = directory.join("shopify.extension.toml");
    if toml_path.exists() {
        return Ok(());
    }
    let content = format!(
        "api_version = \"2024-10\"\n\ntype = \"{extension_type}\"\nname = \"{name}\"\nhandle = \"{handle}\"\nuid = \"{}\"\n",
        deterministic_uid(handle)
    );
    fs::write(toml_path, content)?;
    Ok(())
}

fn clone_shallow(url: &str, destination: &Path) -> Result<(), AppError> {
    fs::create_dir_all(destination)?;
    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            url,
            &destination.display().to_string(),
        ])
        .status()
        .map_err(|e| AppError::message(format!("Failed to run git: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!("git clone failed for {url}")));
    }
    Ok(())
}

fn resolve_flavor_subdir(download: &Path, flavor: Option<&str>) -> Result<PathBuf, AppError> {
    let Some(flavor) = flavor else {
        return Ok(download.to_path_buf());
    };
    // Common layout: template/<flavor>/ or template/flavor-<name>
    for candidate in [
        download.join(flavor),
        download.join(format!("flavor-{flavor}")),
        download.join("template").join(flavor),
    ] {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    Ok(download.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::{load_app, LoadAppOptions};
    use tempfile::tempdir;

    fn demo_app(dir: &Path) -> LoadedApp {
        fs::write(
            dir.join("shopify.app.toml"),
            "name = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        load_app(LoadAppOptions {
            directory: dir.to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap()
    }

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("My Cool Ext!"), "my-cool-ext");
    }

    #[test]
    fn generate_from_local_template() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        let template = dir.path().join("tmpl");
        fs::create_dir_all(&template).unwrap();
        fs::write(
            template.join("shopify.extension.toml.liquid"),
            "type = \"{{type}}\"\nhandle = \"{{handle}}\"\nname = \"{{name}}\"\nuid = \"{{uid}}\"\n",
        )
        .unwrap();
        fs::create_dir_all(template.join("src")).unwrap();
        fs::write(
            template.join("src/index.js.liquid"),
            "export default '{{name}}';\n",
        )
        .unwrap();

        let generated = generate_extension(
            &app,
            GenerateExtensionOptions {
                name: "Cart Transform".into(),
                extension_type: "function".into(),
                flavor: None,
                template: template.display().to_string(),
                local_template: true,
                clone_url: None,
            },
        )
        .unwrap();

        assert_eq!(generated.handle, "cart-transform");
        let toml = fs::read_to_string(
            dir.path()
                .join(&generated.directory)
                .join("shopify.extension.toml"),
        )
        .unwrap();
        assert!(toml.contains("type = \"function\""));
        assert!(toml.contains("handle = \"cart-transform\""));
        assert!(!dir
            .path()
            .join(&generated.directory)
            .join(".shopify.lock")
            .exists());
    }

    #[test]
    fn refuses_existing_extension_directory() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        fs::create_dir_all(dir.path().join("extensions/exists")).unwrap();
        let template = dir.path().join("tmpl");
        fs::create_dir_all(&template).unwrap();
        let err = generate_extension(
            &app,
            GenerateExtensionOptions {
                name: "exists".into(),
                extension_type: "theme".into(),
                flavor: None,
                template: template.display().to_string(),
                local_template: true,
                clone_url: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn flavor_subdir_prefers_named_folder() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("typescript")).unwrap();
        let resolved = resolve_flavor_directory(dir.path(), Some("typescript"));
        assert!(resolved.ends_with("typescript"));
    }

    #[test]
    fn generate_from_flavor_subdirectory() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        let template = dir.path().join("tmpl");
        fs::create_dir_all(template.join("typescript")).unwrap();
        fs::write(
            template.join("typescript/shopify.extension.toml.liquid"),
            "type = \"{{type}}\"\nhandle = \"{{handle}}\"\nflavor = \"{{flavor}}\"\n",
        )
        .unwrap();
        let generated = generate_extension(
            &app,
            GenerateExtensionOptions {
                name: "Flavor Ext".into(),
                extension_type: "ui_extension".into(),
                flavor: Some("typescript".into()),
                template: template.display().to_string(),
                local_template: true,
                clone_url: None,
            },
        )
        .unwrap();
        let toml = fs::read_to_string(
            dir.path()
                .join(&generated.directory)
                .join("shopify.extension.toml"),
        )
        .unwrap();
        assert!(toml.contains("flavor = \"typescript\""));
        assert!(toml.contains("type = \"ui_extension\""));
    }

    #[test]
    fn empty_name_is_rejected() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        let err = ensure_extension_directory(&app, "   ").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn fetch_templates_filters_by_available_specs() {
        use crate::test_support::MockClient;
        use cli_api::ExtensionTemplate;
        let mut client = MockClient::default();
        client.templates = vec![
            ExtensionTemplate {
                identifier: "theme".into(),
                name: "Theme app extension".into(),
                group: Some("Online store".into()),
                url: Some("https://github.com/Shopify/theme-ext".into()),
                types: vec![],
            },
            ExtensionTemplate {
                identifier: "unknown_remote".into(),
                name: "Nope".into(),
                group: None,
                url: None,
                types: vec![],
            },
        ];
        let app = cli_api::MinimalAppIdentifiers {
            id: "1".into(),
            api_key: "k".into(),
            organization_id: "org".into(),
        };
        let result = fetch_extension_templates(&client, &app, &["theme".into()])
            .await
            .unwrap();
        assert_eq!(result.templates.len(), 1);
        assert_eq!(result.templates[0].identifier, "theme");
    }
}
