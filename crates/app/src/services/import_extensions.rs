//! Import dashboard extension registrations into local TOML files.

use crate::error::AppError;
use crate::models::loader::LoadedApp;
use crate::services::admin_link;
use crate::services::flow;
use crate::services::generate::slugify;
use crate::services::marketing_activity;
use crate::services::payments;
use crate::services::subscription_link;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRegistration {
    pub uuid: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub draft_version: Option<ExtensionVersion>,
    pub active_version: Option<ExtensionVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionVersion {
    pub config: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImportExtensionsOptions {
    pub extensions: Vec<ExtensionRegistration>,
    /// When empty, all known migration types are considered.
    pub extension_types: Vec<String>,
    /// Import all matching without interactive selection.
    pub all: bool,
    /// When a local directory exists, overwrite TOML (true) or skip (false).
    pub overwrite_existing: bool,
    /// App `embedded` flag — used when rewriting admin_link URLs.
    pub app_embedded: bool,
}

#[derive(Debug, Clone)]
pub struct ImportedExtension {
    pub uuid: String,
    pub title: String,
    pub directory: PathBuf,
    pub handle: String,
}

/// Types commonly migrated from the Partners dashboard.
pub const DEFAULT_IMPORT_TYPES: &[&str] = &[
    "theme_app_extension",
    "theme",
    "product_subscription",
    "subscription_management",
    "checkout_post_purchase",
    "checkout_ui_extension",
    "web_pixel_extension",
    "admin_link",
    "app_link",
    "bulk_action",
    "flow_action",
    "flow_trigger",
    "flow_action_definition",
    "flow_trigger_definition",
    "flow_trigger_discovery_webhook",
    "function",
    "payments_app",
    "payments_app_credit_card",
    "payments_app_custom_credit_card",
    "payments_app_custom_onsite",
    "payments_app_redeemable",
    "payments_app_card_present",
    "payments_extension",
    "marketing_activity_extension",
    "marketing_activity",
    "subscription_link",
];

pub fn filter_out_imported_extensions(
    app: &LoadedApp,
    extensions: &[ExtensionRegistration],
    env_uuids: &HashMap<String, String>,
) -> Vec<ExtensionRegistration> {
    // Registrations whose UUID already appears in .env are considered imported.
    let imported: std::collections::HashSet<&str> =
        env_uuids.values().map(String::as_str).collect();
    let _ = app;
    extensions
        .iter()
        .filter(|ext| !imported.contains(ext.uuid.as_str()))
        .cloned()
        .collect()
}

fn json_to_toml_string(value: &Value) -> Result<String, AppError> {
    let toml_value = json_to_toml(value)?;
    toml::to_string_pretty(&toml_value)
        .map_err(|e| AppError::message(format!("encode extension toml: {e}")))
}

fn json_to_toml(value: &Value) -> Result<toml::Value, AppError> {
    Ok(match value {
        Value::Null => {
            return Err(AppError::message(
                "json→toml: null values are not supported in TOML",
            ))
        }
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                return Err(AppError::message("json→toml: unsupported number"));
            }
        }
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(arr) => toml::Value::Array(
            arr.iter()
                .filter(|v| !v.is_null())
                .map(json_to_toml)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Object(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                if v.is_null() {
                    continue;
                }
                table.insert(k.clone(), json_to_toml(v)?);
            }
            toml::Value::Table(table)
        }
    })
}

fn inject_uid(mut value: Value, uid: &str) -> Value {
    if let Some(extensions) = value.get_mut("extensions").and_then(|v| v.as_array_mut()) {
        if let Some(first) = extensions.first_mut() {
            if let Some(obj) = first.as_object_mut() {
                obj.insert("uid".into(), Value::String(uid.to_string()));
            }
        }
        return value;
    }
    if let Some(obj) = value.as_object_mut() {
        obj.insert("uid".into(), Value::String(uid.to_string()));
    }
    value
}

/// Build type-specific TOML when a niche builder applies; otherwise fall back to a minimal TOML.
pub fn build_extension_toml(
    ext: &ExtensionRegistration,
    all_extensions: &[ExtensionRegistration],
    app_embedded: bool,
) -> Result<String, AppError> {
    let type_name = ext.type_name.to_lowercase();
    let specialized = match type_name.as_str() {
        "flow_action_definition" | "flow_trigger_definition" | "flow_trigger_discovery_webhook" => {
            Some(flow::build_extension_config(ext)?)
        }
        "payments_app"
        | "payments_app_credit_card"
        | "payments_app_custom_credit_card"
        | "payments_app_custom_onsite"
        | "payments_app_redeemable"
        | "payments_app_card_present"
        | "payments_extension" => Some(payments::build_extension_config(ext, all_extensions)?),
        "app_link" | "bulk_action" | "admin_link" => {
            Some(admin_link::build_extension_config(ext, app_embedded)?)
        }
        "marketing_activity_extension" | "marketing_activity" => {
            Some(marketing_activity::build_extension_config(ext)?)
        }
        "subscription_link" => Some(subscription_link::build_extension_config(ext)?),
        _ => None,
    };

    if let Some(config) = specialized {
        return json_to_toml_string(&inject_uid(config, &ext.uuid));
    }

    Ok(build_generic_extension_toml(ext))
}

fn build_generic_extension_toml(ext: &ExtensionRegistration) -> String {
    let handle = slugify(&ext.title);
    let type_name = normalize_type(&ext.type_name);
    let mut lines = vec![
        format!("type = \"{type_name}\""),
        format!("name = \"{}\"", ext.title.replace('"', "\\\"")),
        format!("handle = \"{handle}\""),
        format!("uid = \"{}\"", ext.uuid),
    ];
    if let Some(config) = ext
        .active_version
        .as_ref()
        .or(ext.draft_version.as_ref())
        .and_then(|v| v.config.as_ref())
    {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(config) {
            if let Some(api_version) = value.get("api_version").and_then(|v| v.as_str()) {
                lines.insert(0, format!("api_version = \"{api_version}\""));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn normalize_type(type_name: &str) -> String {
    match type_name.to_lowercase().as_str() {
        "theme_app_extension" => "theme".into(),
        "subscription_management" => "product_subscription".into(),
        other => other.to_string(),
    }
}

pub fn import_extensions(
    app: &LoadedApp,
    options: ImportExtensionsOptions,
) -> Result<Vec<ImportedExtension>, AppError> {
    let types: Vec<String> = if options.extension_types.is_empty() {
        DEFAULT_IMPORT_TYPES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        options.extension_types.clone()
    };

    let all_extensions = options.extensions.clone();
    let mut to_import: Vec<ExtensionRegistration> = options
        .extensions
        .into_iter()
        .filter(|ext| types.iter().any(|t| t.eq_ignore_ascii_case(&ext.type_name)))
        .collect();

    // Filter already-imported using .env if present.
    let env_uuids = load_dotenv_uuids(&app.directory);
    to_import = filter_out_imported_extensions(app, &to_import, &env_uuids);

    if to_import.is_empty() {
        return Err(AppError::message("No extensions to migrate"));
    }

    let mut imported = Vec::new();
    let mut env_updates: HashMap<String, String> = HashMap::new();

    for ext in to_import {
        let handle = slugify(&ext.title);
        let directory = app.directory.join("extensions").join(&handle);
        if directory.exists() && !options.overwrite_existing {
            continue;
        }
        fs::create_dir_all(&directory)?;
        let toml = build_extension_toml(&ext, &all_extensions, options.app_embedded)?;
        fs::write(directory.join("shopify.extension.toml"), toml)?;
        let env_key = format!("SHOPIFY_{}_ID", handle.to_uppercase().replace('-', "_"));
        env_updates.insert(env_key, ext.uuid.clone());
        imported.push(ImportedExtension {
            uuid: ext.uuid,
            title: ext.title,
            directory: PathBuf::from("extensions").join(&handle),
            handle,
        });
    }

    if imported.is_empty() {
        return Err(AppError::message("No extensions to migrate"));
    }

    merge_dotenv(&app.directory, &env_updates)?;
    Ok(imported)
}

fn load_dotenv_uuids(directory: &std::path::Path) -> HashMap<String, String> {
    let path = directory.join(".env");
    let Ok(raw) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    map
}

fn merge_dotenv(
    directory: &std::path::Path,
    updates: &HashMap<String, String>,
) -> Result<(), AppError> {
    if updates.is_empty() {
        return Ok(());
    }
    let path = directory.join(".env");
    let mut existing = load_dotenv_uuids(directory);
    for (k, v) in updates {
        existing.insert(k.clone(), v.clone());
    }
    let mut lines: Vec<String> = existing
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    lines.sort();
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::{load_app, LoadAppOptions};
    use tempfile::tempdir;

    fn demo_app(dir: &std::path::Path) -> LoadedApp {
        fs::write(
            dir.join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Demo\"\napplication_url = \"https://e.com\"\n",
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
    fn builds_toml_from_registration() {
        let ext = ExtensionRegistration {
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            title: "My Theme".into(),
            type_name: "theme_app_extension".into(),
            draft_version: None,
            active_version: Some(ExtensionVersion {
                config: Some(r#"{"api_version":"2024-10"}"#.into()),
                context: None,
            }),
        };
        let toml = build_extension_toml(&ext, &[], false).unwrap();
        assert!(toml.contains("type = \"theme\""));
        assert!(toml.contains("handle = \"my-theme\""));
        assert!(toml.contains("api_version = \"2024-10\""));
    }

    #[test]
    fn builds_admin_link_toml_via_helper() {
        let ext = ExtensionRegistration {
            uuid: "u".into(),
            title: "Admin link title".into(),
            type_name: "app_link".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(r#"{"text":"Admin link label","url":"https://google.es"}"#.into()),
                context: Some("COLLECTIONS#SHOW".into()),
            }),
            active_version: None,
        };
        let toml = build_extension_toml(&ext, &[], false).unwrap();
        assert!(toml.contains("admin_link"));
        assert!(toml.contains("admin.collection-details.action.link"));
        assert!(toml.contains("uid"));
    }

    #[test]
    fn filters_already_imported() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        let mut env = HashMap::new();
        env.insert("SHOPIFY_X_ID".into(), "uuid-1".into());
        let exts = vec![
            ExtensionRegistration {
                uuid: "uuid-1".into(),
                title: "A".into(),
                type_name: "theme".into(),
                draft_version: None,
                active_version: None,
            },
            ExtensionRegistration {
                uuid: "uuid-2".into(),
                title: "B".into(),
                type_name: "theme".into(),
                draft_version: None,
                active_version: None,
            },
        ];
        let filtered = filter_out_imported_extensions(&app, &exts, &env);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].uuid, "uuid-2");
    }

    #[test]
    fn imports_extensions_to_disk() {
        let dir = tempdir().unwrap();
        let app = demo_app(dir.path());
        let imported = import_extensions(
            &app,
            ImportExtensionsOptions {
                extensions: vec![ExtensionRegistration {
                    uuid: "abc-123".into(),
                    title: "Star Rating".into(),
                    type_name: "theme_app_extension".into(),
                    draft_version: None,
                    active_version: None,
                }],
                extension_types: vec![],
                all: true,
                overwrite_existing: true,
                app_embedded: false,
            },
        )
        .unwrap();
        assert_eq!(imported.len(), 1);
        assert!(dir
            .path()
            .join("extensions/star-rating/shopify.extension.toml")
            .exists());
        let env = fs::read_to_string(dir.path().join(".env")).unwrap();
        assert!(env.contains("SHOPIFY_STAR_RATING_ID=abc-123"));
    }
}
