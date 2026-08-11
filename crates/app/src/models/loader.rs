use crate::error::AppError;
use crate::models::app::{AppConfiguration, AppHiddenConfig};
use crate::models::config_file_naming::get_app_configuration_file_name;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::models::extensions::specification::create_extension_specification;
use crate::models::extensions::specifications::{
    is_config_specification, APP_SCHEMA_KEYS, CONFIG_SPEC_ORDER,
};
use crate::models::identifiers::Identifiers;
use crate::models::project::Project;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LoadAppOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    /// When true, unknown app-config TOML sections are ignored instead of soft-errored.
    pub ignore_unknown_extensions: bool,
}

#[derive(Debug, Clone)]
pub struct WebInstance {
    pub directory: PathBuf,
    pub configuration_path: PathBuf,
    pub roles: Vec<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedApp {
    pub directory: PathBuf,
    pub configuration_path: PathBuf,
    pub configuration: AppConfiguration,
    pub hidden_config: AppHiddenConfig,
    pub extensions: Vec<ExtensionInstance>,
    pub webs: Vec<WebInstance>,
    pub identifiers: Identifiers,
    pub name: String,
    pub errors: Vec<String>,
}

impl LoadedApp {
    pub fn client_id(&self) -> Option<&str> {
        self.configuration.client_id.as_deref()
    }

    pub fn is_linked(&self) -> bool {
        self.configuration.is_linked()
    }

    pub fn all_extensions(&self) -> &[ExtensionInstance] {
        &self.extensions
    }
}

/// Load an app from a project directory (config TOML + extension folders + config modules).
pub fn load_app(options: LoadAppOptions) -> Result<LoadedApp, AppError> {
    let directory = options
        .directory
        .canonicalize()
        .unwrap_or_else(|_| options.directory.clone());
    let project = Project::load(&directory)?;
    let configuration_path = resolve_configuration_path(&project, options.config_name.as_deref())?;

    load_from_config_path(
        &directory,
        &configuration_path,
        project.hidden_config,
        options.ignore_unknown_extensions,
    )
}

fn resolve_configuration_path(
    project: &crate::models::project::Project,
    config_name: Option<&str>,
) -> Result<PathBuf, AppError> {
    if let Some(name) = config_name {
        let config_file = get_app_configuration_file_name(Some(name));
        let path = project.directory.join(&config_file);
        if !path.exists() {
            return Err(AppError::message(format!(
                "Couldn't find an app toml file at {}",
                path.display()
            )));
        }
        return Ok(path);
    }

    project
        .active_config_file(None)
        .ok_or_else(|| {
            AppError::message(format!(
                "Couldn't find an app toml file at {}",
                project
                    .directory
                    .join(get_app_configuration_file_name(None))
                    .display()
            ))
        })
}

fn load_from_config_path(
    directory: &Path,
    configuration_path: &Path,
    hidden_config: AppHiddenConfig,
    ignore_unknown_extensions: bool,
) -> Result<LoadedApp, AppError> {
    let raw = fs::read_to_string(configuration_path)?;
    let configuration: AppConfiguration = toml::from_str(&raw).map_err(|e| {
        AppError::configuration(configuration_path.display().to_string(), e.to_string())
    })?;
    let toml_root: toml::Value = toml::from_str(&raw).map_err(|e| {
        AppError::configuration(configuration_path.display().to_string(), e.to_string())
    })?;

    let name = configuration
        .extra
        .get("handle")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| configuration.name.clone())
        .or_else(|| {
            directory
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "app".into());

    let mut errors = Vec::new();
    let mut extensions = load_folder_extensions(directory, &configuration, &mut errors)?;
    let config_extensions = create_config_extension_instances(
        directory,
        configuration_path,
        &configuration,
        &toml_root,
        ignore_unknown_extensions,
        &mut errors,
    )?;
    extensions.extend(config_extensions);

    validate_unique_handles(&extensions, &mut errors);

    let webs = load_webs(directory, &configuration, &mut errors)?;

    let mut identifiers = Identifiers::new();
    if let Some(ref client_id) = configuration.client_id {
        identifiers = identifiers.with_app(client_id.clone());
    }

    Ok(LoadedApp {
        directory: directory.to_path_buf(),
        configuration_path: configuration_path.to_path_buf(),
        configuration,
        hidden_config,
        extensions,
        webs,
        identifiers,
        name,
        errors,
    })
}

fn validate_unique_handles(extensions: &[ExtensionInstance], errors: &mut Vec<String>) {
    let mut seen: HashSet<String> = HashSet::new();
    for ext in extensions {
        if !seen.insert(ext.handle.clone()) {
            errors.push(format!(
                "Duplicate extension handle '{}'. Handles must be unique within an app.",
                ext.handle
            ));
        }
    }
}

fn load_folder_extensions(
    directory: &Path,
    configuration: &AppConfiguration,
    errors: &mut Vec<String>,
) -> Result<Vec<ExtensionInstance>, AppError> {
    let mut extensions = Vec::new();
    let toml_paths = discover_extension_tomls(directory, configuration)?;

    for toml_path in toml_paths {
        let ext_dir = toml_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| directory.to_path_buf());
        match load_extension_instances_from_file(&ext_dir, &toml_path) {
            Ok(mut instances) => extensions.append(&mut instances),
            Err(e) => errors.push(format!("{}: {e}", toml_path.display())),
        }
    }

    Ok(extensions)
}

fn discover_extension_tomls(
    directory: &Path,
    configuration: &AppConfiguration,
) -> Result<Vec<PathBuf>, AppError> {
    let mut found = Vec::new();
    let roots = configuration.normalized_extension_directories();

    for root in roots {
        let pattern = root.trim_end_matches(['/', '\\']);
        let recursive = pattern.contains("**") || pattern.ends_with('*');
        let base = pattern
            .trim_end_matches("**")
            .trim_end_matches('*')
            .trim_end_matches(['/', '\\']);
        let root_path = if base.is_empty() {
            directory.to_path_buf()
        } else {
            directory.join(base)
        };

        if root_path.is_file() && is_extension_toml_name(&root_path) {
            found.push(root_path);
            continue;
        }

        if !root_path.is_dir() {
            continue;
        }

        // Match `dir/*.extension.toml` at the root itself.
        collect_extension_tomls_in_dir(&root_path, recursive, &mut found)?;
    }

    found.sort();
    found.dedup();
    Ok(found)
}

fn is_extension_toml_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.ends_with(".extension.toml") || n == "shopify.extension.toml")
        .unwrap_or(false)
        || matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some(
                "shopify.ui.extension.toml"
                    | "shopify.function.extension.toml"
                    | "shopify.theme.extension.toml"
            )
        )
}

fn collect_extension_tomls_in_dir(
    dir: &Path,
    recursive: bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "node_modules")
    {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_file() && is_extension_toml_name(&path) {
            out.push(path);
        } else if file_type.is_dir() {
            // Always scan one level of child directories (extensions/<name>/...).
            // When recursive (**), descend further.
            let child_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if child_name == "node_modules" {
                continue;
            }
            // Child dir may contain the toml directly.
            for name in [
                "shopify.extension.toml",
                "shopify.ui.extension.toml",
                "shopify.function.extension.toml",
                "shopify.theme.extension.toml",
            ] {
                let candidate = path.join(name);
                if candidate.exists() {
                    out.push(candidate);
                }
            }
            // Also any *.extension.toml in the child.
            if let Ok(entries) = fs::read_dir(&path) {
                for child in entries.flatten() {
                    let cp = child.path();
                    if cp.is_file() && is_extension_toml_name(&cp) {
                        out.push(cp);
                    }
                }
            }
            if recursive {
                collect_extension_tomls_in_dir(&path, true, out)?;
            }
        }
    }
    Ok(())
}

fn load_extension_instances_from_file(
    directory: &Path,
    configuration_path: &Path,
) -> Result<Vec<ExtensionInstance>, AppError> {
    let raw = fs::read_to_string(configuration_path)?;
    let table: toml::Value = toml::from_str(&raw)?;
    let json = toml_value_to_json(&table);

    if let Some(Value::Array(items)) = json.get("extensions").cloned() {
        let globals = match &json {
            Value::Object(map) => map
                .iter()
                .filter(|(k, _)| k.as_str() != "extensions")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<_, _>>(),
            _ => HashMap::new(),
        };
        let mut instances = Vec::new();
        for item in items {
            let Value::Object(local) = item else {
                continue;
            };
            let mut merged = globals.clone();
            for (k, v) in local {
                merged.insert(k, v);
            }
            let handle = merged
                .get("handle")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::message(format!(
                        "{}: unified extension entries require a handle",
                        configuration_path.display()
                    ))
                })?
                .to_string();
            instances.push(build_extension_instance(
                directory,
                configuration_path,
                handle,
                merged,
            )?);
        }
        return Ok(instances);
    }

    let obj = match json {
        Value::Object(map) => map.into_iter().collect::<HashMap<_, _>>(),
        _ => HashMap::new(),
    };

    let handle = obj
        .get("handle")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            directory
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "extension".into())
        });

    Ok(vec![build_extension_instance(
        directory,
        configuration_path,
        handle,
        obj,
    )?])
}

fn build_extension_instance(
    directory: &Path,
    configuration_path: &Path,
    handle: String,
    obj: HashMap<String, Value>,
) -> Result<ExtensionInstance, AppError> {
    let type_name = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("ui_extension");

    let specification = create_extension_specification(type_name).ok_or_else(|| {
        AppError::message(format!("Unknown extension type '{type_name}'"))
    })?;

    let mut instance = ExtensionInstance::new(
        handle,
        directory.to_path_buf(),
        configuration_path.to_path_buf(),
        obj,
        specification,
    );
    instance.uid = instance
        .configuration
        .get("uid")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    instance.entry_path = find_entry_path(directory, &instance);
    Ok(instance)
}

fn find_entry_path(directory: &Path, instance: &ExtensionInstance) -> Option<PathBuf> {
    let candidates = if instance.is_function_extension() {
        vec![
            "src/index.js",
            "src/index.ts",
            "src/main.rs",
            "src/index.rs",
        ]
    } else {
        vec![
            "src/index.js",
            "src/index.jsx",
            "src/index.ts",
            "src/index.tsx",
            "index.js",
            "index.jsx",
            "index.ts",
            "index.tsx",
        ]
    };
    for rel in candidates {
        let p = directory.join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn create_config_extension_instances(
    directory: &Path,
    configuration_path: &Path,
    configuration: &AppConfiguration,
    toml_root: &toml::Value,
    ignore_unknown_extensions: bool,
    errors: &mut Vec<String>,
) -> Result<Vec<ExtensionInstance>, AppError> {
    let mut instances = Vec::new();
    let mut consumed_keys: HashSet<String> = HashSet::new();

    let table = match toml_root.as_table() {
        Some(t) => t,
        None => return Ok(instances),
    };

    for &spec_id in CONFIG_SPEC_ORDER {
        if spec_id == "webhook_subscription" {
            // Created from [[webhooks.subscriptions]] expansion below.
            continue;
        }

        let Some(section) = table.get(spec_id) else {
            // Aliases: pos → point_of_sale historically stored as [pos]
            if spec_id == "point_of_sale" {
                if let Some(pos) = table.get("pos") {
                    consumed_keys.insert("pos".into());
                    instances.push(config_module_instance(
                        directory,
                        configuration_path,
                        spec_id,
                        pos,
                    )?);
                }
            }
            continue;
        };

        consumed_keys.insert(spec_id.to_string());
        instances.push(config_module_instance(
            directory,
            configuration_path,
            spec_id,
            section,
        )?);

        if spec_id == "webhooks" {
            if let Some(subs) = section.get("subscriptions").and_then(|v| v.as_array()) {
                for (idx, sub) in subs.iter().enumerate() {
                    let topics = extract_webhook_topics(sub);
                    if topics.is_empty() {
                        instances.push(webhook_subscription_instance(
                            directory,
                            configuration_path,
                            sub,
                            idx,
                            None,
                        )?);
                    } else {
                        for topic in topics {
                            instances.push(webhook_subscription_instance(
                                directory,
                                configuration_path,
                                sub,
                                idx,
                                Some(&topic),
                            )?);
                        }
                    }
                }
            }
        }
    }

    // Flag unsupported top-level sections.
    if !ignore_unknown_extensions {
        let mut unused = Vec::new();
        for key in table.keys() {
            if APP_SCHEMA_KEYS.contains(&key.as_str()) || consumed_keys.contains(key.as_str()) {
                continue;
            }
            if is_config_specification(key) {
                continue;
            }
            // Known aliases already handled
            if key == "pos" {
                continue;
            }
            unused.push(key.clone());
        }
        if !unused.is_empty() {
            unused.sort();
            errors.push(format!(
                "Unsupported section(s) in app configuration: {}",
                unused.join(", ")
            ));
        }
    }

    let _ = configuration; // reserved for future filtering
    Ok(instances)
}

fn config_module_instance(
    directory: &Path,
    configuration_path: &Path,
    spec_id: &str,
    section: &toml::Value,
) -> Result<ExtensionInstance, AppError> {
    let specification = create_extension_specification(spec_id)
        .ok_or_else(|| AppError::message(format!("Unknown config module '{spec_id}'")))?;
    let json = toml_value_to_json(section);
    let mut obj = match json {
        Value::Object(map) => map.into_iter().collect::<HashMap<_, _>>(),
        other => {
            let mut m = HashMap::new();
            m.insert("value".into(), other);
            m
        }
    };
    obj.insert("type".into(), Value::String(spec_id.to_string()));
    obj.insert("handle".into(), Value::String(spec_id.to_string()));

    Ok(ExtensionInstance::new(
        spec_id.to_string(),
        directory.to_path_buf(),
        configuration_path.to_path_buf(),
        obj,
        specification,
    ))
}

fn extract_webhook_topics(sub: &toml::Value) -> Vec<String> {
    if let Some(arr) = sub.get("topics").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect();
    }
    if let Some(t) = sub.get("topics").and_then(|v| v.as_str()) {
        return vec![t.to_string()];
    }
    if let Some(t) = sub.get("topic").and_then(|v| v.as_str()) {
        return vec![t.to_string()];
    }
    Vec::new()
}

fn webhook_subscription_instance(
    directory: &Path,
    configuration_path: &Path,
    sub: &toml::Value,
    idx: usize,
    topic: Option<&str>,
) -> Result<ExtensionInstance, AppError> {
    let specification = create_extension_specification("webhook_subscription").ok_or_else(|| {
        AppError::message("Missing webhook_subscription specification".to_string())
    })?;
    let json = toml_value_to_json(sub);
    let mut obj = match json {
        Value::Object(map) => map.into_iter().collect::<HashMap<_, _>>(),
        _ => HashMap::new(),
    };
    if let Some(topic) = topic {
        obj.insert("topic".into(), Value::String(topic.to_string()));
    }
    obj.insert(
        "type".into(),
        Value::String("webhook_subscription".into()),
    );
    let handle = topic
        .map(|t| format!("webhook-{}", t.replace('/', "-").to_lowercase()))
        .unwrap_or_else(|| format!("webhook-subscription-{idx}"));
    obj.insert("handle".into(), Value::String(handle.clone()));

    Ok(ExtensionInstance::new(
        handle,
        directory.to_path_buf(),
        configuration_path.to_path_buf(),
        obj,
        specification,
    ))
}

fn load_webs(
    directory: &Path,
    configuration: &AppConfiguration,
    errors: &mut Vec<String>,
) -> Result<Vec<WebInstance>, AppError> {
    let mut webs = Vec::new();
    let search_roots: Vec<PathBuf> = configuration
        .web_directories
        .as_ref()
        .map(|dirs| dirs.iter().map(|d| directory.join(d)).collect())
        .unwrap_or_else(|| vec![directory.to_path_buf()]);

    for root in search_roots {
        collect_webs(&root, &mut webs)?;
    }

    for role in ["frontend", "backend"] {
        let count = webs
            .iter()
            .filter(|w| w.roles.iter().any(|r| r == role))
            .count();
        if count > 1 {
            errors.push(format!(
                "Multiple webs with role '{role}' found. Only one is allowed."
            ));
        }
    }

    Ok(webs)
}

fn collect_webs(dir: &Path, out: &mut Vec<WebInstance>) -> Result<(), AppError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let candidate = dir.join("shopify.web.toml");
    if candidate.exists() {
        if let Ok(web) = parse_web(&dir.to_path_buf(), &candidate) {
            out.push(web);
        }
    }
    // Shallow scan of immediate children (web/, frontend/, etc.)
    for entry in fs::read_dir(dir)?.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let child = entry.path();
        let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "node_modules" || name == "extensions" || name.starts_with('.') {
            continue;
        }
        let toml_path = child.join("shopify.web.toml");
        if toml_path.exists() {
            if let Ok(web) = parse_web(&child, &toml_path) {
                out.push(web);
            }
        }
    }
    Ok(())
}

fn parse_web(directory: &PathBuf, configuration_path: &Path) -> Result<WebInstance, AppError> {
    let raw = fs::read_to_string(configuration_path)?;
    let value: toml::Value = toml::from_str(&raw)?;
    let roles = value
        .get("roles")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_str().map(str::to_string))
                .collect()
        })
        .or_else(|| {
            value
                .get("type")
                .and_then(|v| v.as_str())
                .map(|t| vec![t.to_string()])
        })
        .unwrap_or_default();
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(WebInstance {
        directory: directory.clone(),
        configuration_path: configuration_path.to_path_buf(),
        roles,
        name,
    })
}

fn toml_value_to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_value_to_json).collect()),
        toml::Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table {
                map.insert(k.clone(), toml_value_to_json(v));
            }
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_app(dir: &Path, toml: &str) {
        fs::write(dir.join("shopify.app.toml"), toml).unwrap();
    }

    #[test]
    fn load_app_parses_config_and_extensions() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
client_id = "gid://app/1"
name = "Demo"
application_url = "https://example.com"

[access_scopes]
scopes = "write_products"
"#,
        );
        let ext_dir = dir.path().join("extensions/my-theme");
        fs::create_dir_all(&ext_dir).unwrap();
        let mut f = fs::File::create(ext_dir.join("shopify.extension.toml")).unwrap();
        writeln!(
            f,
            r#"
type = "theme"
handle = "my-theme"
name = "My Theme Ext"
"#
        )
        .unwrap();

        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();

        assert!(app.is_linked());
        assert_eq!(app.name, "Demo");
        assert!(app.extensions.iter().any(|e| e.is_theme_extension()));
        let theme = app
            .extensions
            .iter()
            .find(|e| e.is_theme_extension())
            .unwrap();
        assert_eq!(theme.bundle_url(), "dist/theme/my-theme");
    }

    #[test]
    fn load_app_missing_config_errors() {
        let dir = tempdir().unwrap();
        let err = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("Couldn't find"));
    }

    #[test]
    fn explicit_missing_config_does_not_fallback() {
        let dir = tempdir().unwrap();
        write_app(dir.path(), "name = \"Demo\"\n");
        let err = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: Some("production".into()),
            ignore_unknown_extensions: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("Couldn't find"));
        assert!(err.to_string().contains("production"));
    }

    #[test]
    fn prefers_handle_over_name() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Pretty Name"
handle = "pretty-handle"
application_url = "https://example.com"
"#,
        );
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert_eq!(app.name, "pretty-handle");
    }

    #[test]
    fn loads_config_modules_from_app_toml() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://example.com"

[pos]
embedded = true

[branding]
name = "Brand"
"#,
        );
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        assert!(app
            .extensions
            .iter()
            .any(|e| e.specification.identifier == "point_of_sale"));
        assert!(app
            .extensions
            .iter()
            .any(|e| e.specification.identifier == "branding"));
    }

    #[test]
    fn expands_webhook_topics_into_subscriptions() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://example.com"

[webhooks]
api_version = "2024-10"

[[webhooks.subscriptions]]
topics = ["orders/create", "orders/updated"]
uri = "/webhooks"
"#,
        );
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        let subs: Vec<_> = app
            .extensions
            .iter()
            .filter(|e| e.specification.identifier == "webhook_subscription")
            .collect();
        assert_eq!(subs.len(), 2);
        assert!(app
            .extensions
            .iter()
            .any(|e| e.specification.identifier == "webhooks"));
    }

    #[test]
    fn unsupported_section_soft_errors() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://example.com"

[something_else]
foo = 1
"#,
        );
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        assert!(app
            .errors
            .iter()
            .any(|e| e.contains("Unsupported section") && e.contains("something_else")));
    }

    #[test]
    fn ignore_unknown_extensions_suppresses_unsupported() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://example.com"

[something_else]
foo = 1
"#,
        );
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert!(app.errors.is_empty());
    }

    #[test]
    fn unified_extensions_array_merges_globals() {
        let dir = tempdir().unwrap();
        write_app(dir.path(), "name = \"Demo\"\napplication_url = \"https://e.com\"\n");
        let ext_dir = dir.path().join("extensions/multi");
        fs::create_dir_all(&ext_dir).unwrap();
        fs::write(
            ext_dir.join("shopify.extension.toml"),
            r#"
api_version = "2024-10"

[[extensions]]
type = "theme"
handle = "one"
name = "One"

[[extensions]]
type = "theme"
handle = "two"
name = "Two"
description = "override"
"#,
        )
        .unwrap();

        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        let themes: Vec<_> = app
            .extensions
            .iter()
            .filter(|e| e.is_theme_extension())
            .collect();
        assert_eq!(themes.len(), 2);
        assert_eq!(
            themes[0].configuration.get("api_version").and_then(|v| v.as_str()),
            Some("2024-10")
        );
        assert_eq!(
            themes
                .iter()
                .find(|e| e.handle == "two")
                .unwrap()
                .configuration
                .get("description")
                .and_then(|v| v.as_str()),
            Some("override")
        );
    }

    #[test]
    fn nested_extension_directories_glob() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://e.com"
extension_directories = ["extensions/**"]
"#,
        );
        let deep = dir.path().join("extensions/nested/deep/my-ext");
        fs::create_dir_all(&deep).unwrap();
        fs::write(
            deep.join("shopify.extension.toml"),
            "type = \"theme\"\nhandle = \"deep-ext\"\n",
        )
        .unwrap();

        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert!(app.extensions.iter().any(|e| e.handle == "deep-ext"));
    }

    #[test]
    fn custom_extension_directory() {
        let dir = tempdir().unwrap();
        write_app(
            dir.path(),
            r#"
name = "Demo"
application_url = "https://e.com"
extension_directories = ["custom_extensions"]
"#,
        );
        let ext = dir.path().join("custom_extensions/foo");
        fs::create_dir_all(&ext).unwrap();
        fs::write(
            ext.join("shopify.extension.toml"),
            "type = \"function\"\nhandle = \"foo\"\n",
        )
        .unwrap();
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert!(app.extensions.iter().any(|e| e.is_function_extension()));
    }

    #[test]
    fn duplicate_handles_soft_error() {
        let dir = tempdir().unwrap();
        write_app(dir.path(), "name = \"Demo\"\napplication_url = \"https://e.com\"\n");
        for name in ["a", "b"] {
            let ext = dir.path().join("extensions").join(name);
            fs::create_dir_all(&ext).unwrap();
            fs::write(
                ext.join("shopify.extension.toml"),
                "type = \"theme\"\nhandle = \"same\"\n",
            )
            .unwrap();
        }
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert!(app.errors.iter().any(|e| e.contains("Duplicate extension handle")));
    }

    #[test]
    fn loads_web_and_detects_role_conflict() {
        let dir = tempdir().unwrap();
        write_app(dir.path(), "name = \"Demo\"\napplication_url = \"https://e.com\"\n");
        for name in ["web1", "web2"] {
            let web = dir.path().join(name);
            fs::create_dir_all(&web).unwrap();
            fs::write(
                web.join("shopify.web.toml"),
                "roles = [\"frontend\"]\nname = \"x\"\n",
            )
            .unwrap();
        }
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: true,
        })
        .unwrap();
        assert_eq!(app.webs.len(), 2);
        assert!(app
            .errors
            .iter()
            .any(|e| e.contains("Multiple webs") && e.contains("frontend")));
    }
}
