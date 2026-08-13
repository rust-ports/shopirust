//! Write / patch shopify.app.toml helpers (upstream `write-app-configuration-file` / `patch-app-configuration-file`).

use crate::error::AppError;
use crate::models::AppConfiguration;
use serde_json::Value;
use std::fs;
use std::path::Path;

const CONFIG_HEADER: &str =
    "# Learn more about configuring your app at https://shopify.dev/docs/apps/tools/cli/configuration\n\n";
const ACCESS_SCOPES_COMMENT: &str =
    "# Learn more at https://shopify.dev/docs/apps/tools/cli/configuration#access_scopes\n";

/// Recursively drop empty objects (but keep empty arrays / null).
pub fn strip_empty_objects(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let stripped = strip_empty_objects(v);
                let drop = matches!(&stripped, Value::Object(m) if m.is_empty());
                if !drop {
                    out.insert(k, stripped);
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(strip_empty_objects).collect()),
        other => other,
    }
}

/// Serialize an [`AppConfiguration`] to TOML and write it atomically.
pub fn write_app_configuration_file(
    path: &Path,
    configuration: &AppConfiguration,
) -> Result<(), AppError> {
    let value = serde_json::to_value(configuration)
        .map_err(|e| AppError::message(format!("serialize app config: {e}")))?;
    let value = strip_empty_objects(value);
    let toml_value = json_to_toml(&value)?;
    let mut rendered = toml::to_string_pretty(&toml_value)
        .map_err(|e| AppError::message(format!("encode app config toml: {e}")))?;
    if !rendered.starts_with('#') {
        rendered = format!("{CONFIG_HEADER}{rendered}");
    }
    if rendered.contains("[access_scopes]") && !rendered.contains("configuration#access_scopes") {
        rendered = rendered.replace(
            "[access_scopes]\n",
            &format!("[access_scopes]\n{ACCESS_SCOPES_COMMENT}"),
        );
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, rendered)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Patch hidden CLI config keyed by client_id (upstream `patchAppHiddenConfigFile`).
pub fn patch_app_hidden_config_file(
    path: &Path,
    client_id: &str,
    patch: &Value,
) -> Result<(), AppError> {
    let mut root = if path.is_file() {
        let raw = fs::read_to_string(path)?;
        serde_json::from_str::<Value>(&raw).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };
    if !root.is_object() {
        root = Value::Object(Default::default());
    }
    let current = root
        .get(client_id)
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    let merged = crate::services::config::deep_merge(current, patch.clone());
    if let Some(obj) = root.as_object_mut() {
        obj.insert(client_id.to_string(), merged);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Patch selected top-level keys into an existing TOML file (or create it).
pub fn patch_app_configuration_file(
    path: &Path,
    patch: &Value,
) -> Result<AppConfiguration, AppError> {
    let mut current = if path.is_file() {
        let raw = fs::read_to_string(path)?;
        let table: toml::Value = toml::from_str(&raw)?;
        let json = toml_to_json(&table);
        serde_json::from_value(json)
            .map_err(|e| AppError::message(format!("parse app config: {e}")))?
    } else {
        AppConfiguration::default()
    };

    apply_json_patch(&mut current, patch)?;
    write_app_configuration_file(path, &current)?;
    Ok(current)
}

/// Ensure every extension TOML has a `uid` field when known.
pub fn add_uid_to_extension_toml(path: &Path, uid: &str) -> Result<(), AppError> {
    let raw = fs::read_to_string(path)?;
    if raw.lines().any(|l| l.trim_start().starts_with("uid ")) {
        return Ok(());
    }
    let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
    // Insert after handle or at top of first table.
    let insert_at = lines
        .iter()
        .position(|l| l.trim_start().starts_with("handle "))
        .map(|i| i + 1)
        .unwrap_or(0);
    lines.insert(insert_at, format!("uid = \"{uid}\""));
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

fn apply_json_patch(config: &mut AppConfiguration, patch: &Value) -> Result<(), AppError> {
    let mut current =
        serde_json::to_value(&*config).map_err(|e| AppError::message(e.to_string()))?;
    if let (Some(cur), Some(p)) = (current.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            cur.insert(k.clone(), v.clone());
        }
    }
    *config = serde_json::from_value(current)
        .map_err(|e| AppError::message(format!("apply patch: {e}")))?;
    Ok(())
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

fn toml_to_json(value: &toml::Value) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_patch_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        let cfg = AppConfiguration {
            name: Some("Demo".into()),
            client_id: Some("abc".into()),
            ..Default::default()
        };
        write_app_configuration_file(&path, &cfg).unwrap();

        patch_app_configuration_file(
            &path,
            &serde_json::json!({ "application_url": "https://example.com" }),
        )
        .unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("Demo") || raw.contains("name"));
        assert!(raw.contains("example.com"));
    }

    #[test]
    fn add_uid_once() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.extension.toml");
        fs::write(&path, "type = \"theme\"\nhandle = \"x\"\n").unwrap();
        add_uid_to_extension_toml(&path, "uid-1").unwrap();
        add_uid_to_extension_toml(&path, "uid-2").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw.matches("uid =").count(), 1);
        assert!(raw.contains("uid-1"));
    }

    #[test]
    fn write_includes_header_and_drops_empty_objects() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        let mut cfg = AppConfiguration {
            name: Some("my app".into()),
            client_id: Some("api-key".into()),
            application_url: Some("https://myapp.com/".into()),
            embedded: Some(true),
            ..Default::default()
        };
        cfg.extra.insert(
            "webhooks".into(),
            serde_json::json!({"api_version": "2023-07", "privacy_compliance": {}}),
        );
        cfg.extra.insert(
            "auth".into(),
            serde_json::json!({"redirect_urls": ["https://example.com/redirect"]}),
        );
        cfg.extra.insert(
            "access_scopes".into(),
            serde_json::json!({"scopes": "read_products"}),
        );
        write_app_configuration_file(&path, &cfg).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Learn more about configuring your app"));
        assert!(content.contains("client_id = \"api-key\""));
        assert!(content.contains("redirect_urls"));
        assert!(!content.contains("privacy_compliance"));
        assert!(content.contains("configuration#access_scopes"));
    }

    #[test]
    fn write_preserves_empty_arrays() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        let mut cfg = AppConfiguration {
            client_id: Some("api-key".into()),
            ..Default::default()
        };
        cfg.extra
            .insert("auth".into(), serde_json::json!({"redirect_urls": []}));
        write_app_configuration_file(&path, &cfg).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("redirect_urls"));
    }

    #[test]
    fn write_survives_type_mismatched_extra() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        let mut cfg = AppConfiguration {
            client_id: Some("api-key".into()),
            ..Default::default()
        };
        cfg.extra.insert(
            "auth".into(),
            serde_json::json!({"redirect_urls": "not-an-array"}),
        );
        write_app_configuration_file(&path, &cfg).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("api-key"));
    }

    #[test]
    fn strip_empty_objects_removes_nested() {
        assert_eq!(
            strip_empty_objects(serde_json::json!({"name": "hello", "empty": {}})),
            serde_json::json!({"name": "hello"})
        );
        assert_eq!(
            strip_empty_objects(serde_json::json!({"outer": {"inner": {}}})),
            serde_json::json!({})
        );
        assert_eq!(
            strip_empty_objects(serde_json::json!({"items": []})),
            serde_json::json!({"items": []})
        );
        assert_eq!(strip_empty_objects(Value::Null), Value::Null);
        assert_eq!(
            strip_empty_objects(serde_json::json!({"items": [{"val": 1, "empty": {}}, {"val": 2}]})),
            serde_json::json!({"items": [{"val": 1}, {"val": 2}]})
        );
    }

    #[test]
    fn patch_preserves_unrelated_keys() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        fs::write(
            &path,
            "client_id = \"abc\"\nname = \"Keep Me\"\napplication_url = \"https://old.example\"\n",
        )
        .unwrap();
        patch_app_configuration_file(
            &path,
            &serde_json::json!({ "application_url": "https://new.example" }),
        )
        .unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("Keep Me"));
        assert!(raw.contains("https://new.example"));
        assert!(!raw.contains("https://old.example"));
    }

    #[test]
    fn patch_creates_file_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shopify.app.toml");
        patch_app_configuration_file(&path, &serde_json::json!({ "name": "Created" })).unwrap();
        assert!(path.is_file());
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("Created"));
    }

    #[test]
    fn hidden_config_creates_and_preserves_other_clients() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".project.json");
        patch_app_hidden_config_file(
            &path,
            "12345",
            &serde_json::json!({"dev_store_url": "test-store.myshopify.com"}),
        )
        .unwrap();
        patch_app_hidden_config_file(
            &path,
            "67890",
            &serde_json::json!({"dev_store_url": "store-2.myshopify.com"}),
        )
        .unwrap();
        patch_app_hidden_config_file(
            &path,
            "12345",
            &serde_json::json!({"dev_store_url": "updated-store.myshopify.com"}),
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            parsed["12345"]["dev_store_url"].as_str(),
            Some("updated-store.myshopify.com")
        );
        assert_eq!(
            parsed["67890"]["dev_store_url"].as_str(),
            Some("store-2.myshopify.com")
        );
    }
}
