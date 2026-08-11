//! Write / patch shopify.app.toml helpers (upstream `write-app-configuration-file` / `patch-app-configuration-file`).

use crate::error::AppError;
use crate::models::AppConfiguration;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Serialize an [`AppConfiguration`] to TOML and write it atomically.
pub fn write_app_configuration_file(
    path: &Path,
    configuration: &AppConfiguration,
) -> Result<(), AppError> {
    let value = serde_json::to_value(configuration)
        .map_err(|e| AppError::message(format!("serialize app config: {e}")))?;
    let toml_value = json_to_toml(&value)?;
    let rendered = toml::to_string_pretty(&toml_value)
        .map_err(|e| AppError::message(format!("encode app config toml: {e}")))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, rendered)?;
    fs::rename(&tmp, path)?;
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
}
