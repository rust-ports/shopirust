use crate::error::AppError;
use crate::models::app::{AppConfiguration, AppHiddenConfig};
use crate::models::config_file_naming::get_app_configuration_file_name;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::models::extensions::specification::create_extension_specification;
use crate::models::identifiers::Identifiers;
use crate::models::project::Project;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LoadAppOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedApp {
    pub directory: PathBuf,
    pub configuration_path: PathBuf,
    pub configuration: AppConfiguration,
    pub hidden_config: AppHiddenConfig,
    pub extensions: Vec<ExtensionInstance>,
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
}

/// Load an app from a project directory (config TOML + extension folders).
pub fn load_app(options: LoadAppOptions) -> Result<LoadedApp, AppError> {
    let directory = options
        .directory
        .canonicalize()
        .unwrap_or(options.directory);
    let project = Project::load(&directory)?;
    let config_file = get_app_configuration_file_name(options.config_name.as_deref());
    let configuration_path = directory.join(&config_file);

    if !configuration_path.exists() {
        // Fall back to any shopify.app*.toml discovered by the project.
        if let Some(path) = project.config_files.first() {
            return load_from_config_path(&directory, path, project.hidden_config);
        }
        return Err(AppError::message(format!(
            "Couldn't find an app toml file at {}",
            configuration_path.display()
        )));
    }

    load_from_config_path(&directory, &configuration_path, project.hidden_config)
}

fn load_from_config_path(
    directory: &Path,
    configuration_path: &Path,
    hidden_config: AppHiddenConfig,
) -> Result<LoadedApp, AppError> {
    let raw = fs::read_to_string(configuration_path)?;
    let configuration: AppConfiguration = toml::from_str(&raw).map_err(|e| {
        AppError::configuration(configuration_path.display().to_string(), e.to_string())
    })?;

    let name = configuration
        .name
        .clone()
        .or_else(|| {
            directory
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "app".into());

    let mut errors = Vec::new();
    let extensions = load_extensions(directory, &configuration, &mut errors)?;

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
        identifiers,
        name,
        errors,
    })
}

fn load_extensions(
    directory: &Path,
    configuration: &AppConfiguration,
    errors: &mut Vec<String>,
) -> Result<Vec<ExtensionInstance>, AppError> {
    let mut extensions = Vec::new();
    let search_roots = configuration.normalized_extension_directories();

    for root in search_roots {
        let root_path = directory.join(root.trim_end_matches('*').trim_end_matches('/'));
        if !root_path.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&root_path)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let ext_dir = entry.path();
            let toml_path = find_extension_toml(&ext_dir);
            let Some(toml_path) = toml_path else {
                continue;
            };
            match load_extension_instance(&ext_dir, &toml_path) {
                Ok(ext) => extensions.push(ext),
                Err(e) => errors.push(format!("{}: {e}", toml_path.display())),
            }
        }
    }

    Ok(extensions)
}

fn find_extension_toml(dir: &Path) -> Option<PathBuf> {
    for name in [
        "shopify.extension.toml",
        "shopify.ui.extension.toml",
        "shopify.function.extension.toml",
        "shopify.theme.extension.toml",
    ] {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn load_extension_instance(
    directory: &Path,
    configuration_path: &Path,
) -> Result<ExtensionInstance, AppError> {
    let raw = fs::read_to_string(configuration_path)?;
    let table: toml::Value = toml::from_str(&raw)?;
    let json = toml_value_to_json(&table);
    let obj = match json {
        Value::Object(map) => map.into_iter().collect::<HashMap<_, _>>(),
        _ => HashMap::new(),
    };

    let type_name = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("ui_extension");

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

    let specification = create_extension_specification(type_name)
        .ok_or_else(|| AppError::message(format!("Unknown extension type '{type_name}'")))?;

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
    Ok(instance)
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
        })
        .unwrap();

        assert!(app.is_linked());
        assert_eq!(app.name, "Demo");
        assert_eq!(app.extensions.len(), 1);
        assert!(app.extensions[0].is_theme_extension());
        assert_eq!(app.extensions[0].bundle_url(), "dist/theme/my-theme");
    }

    #[test]
    fn load_app_missing_config_errors() {
        let dir = tempdir().unwrap();
        let err = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("Couldn't find"));
    }
}
