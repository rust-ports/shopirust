//! Shared extension configuration schemas (mirrors upstream `schemas.ts`).

use crate::error::AppError;
use serde_json::Value;

pub const MAX_EXTENSION_HANDLE_LENGTH: usize = 50;
pub const MAX_UID_LENGTH: usize = 250;

/// First-class TOML fields stripped before contract deploy payloads.
pub const FIRST_CLASS_FIELDS: &[&str] = &["type", "handle", "uid", "path", "extensions"];

/// Validate optional handle / uid / name base fields from a config object.
pub fn validate_base_fields(config: &Value, require_handle: bool) -> Result<(), AppError> {
    let obj = config
        .as_object()
        .ok_or_else(|| AppError::message("extension config must be an object"))?;

    if let Some(handle) = obj.get("handle").and_then(|v| v.as_str()) {
        validate_handle(handle)?;
    } else if require_handle {
        return Err(AppError::message("Handle can't be empty"));
    }

    if let Some(uid) = obj.get("uid").and_then(|v| v.as_str()) {
        validate_uid(uid)?;
    }

    Ok(())
}

pub fn validate_handle(handle: &str) -> Result<(), AppError> {
    let handle = handle.trim();
    if handle.is_empty() {
        return Err(AppError::message("Handle can't be empty"));
    }
    if handle.len() > MAX_EXTENSION_HANDLE_LENGTH {
        return Err(AppError::message(format!(
            "Handle can't exceed {MAX_EXTENSION_HANDLE_LENGTH} characters"
        )));
    }
    if !handle
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AppError::message(
            "Handle can only contain alphanumeric characters and hyphens",
        ));
    }
    if handle.starts_with('-') || handle.ends_with('-') {
        return Err(AppError::message(
            "Handle can't start or end with a hyphen",
        ));
    }
    Ok(())
}

pub fn validate_uid(uid: &str) -> Result<(), AppError> {
    let uid = uid.trim();
    if uid.is_empty() {
        return Err(AppError::message("UID can't be empty"));
    }
    if uid.len() > MAX_UID_LENGTH {
        return Err(AppError::message(format!(
            "UID can't exceed {MAX_UID_LENGTH} characters"
        )));
    }
    if !uid.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '-' | '$' | '{' | '}' | '.' | '(' | ')' | '_' | '`')
    }) {
        return Err(AppError::message(
            "UID can only contain alphanumeric characters and hyphens",
        ));
    }
    if uid.starts_with('-') || uid.ends_with('-') {
        return Err(AppError::message("UID can't start or end with a hyphen"));
    }
    Ok(())
}

/// Remove first-class fields from a config object (contract deploy).
pub fn config_without_first_class_fields(config: &Value) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(obj) = config.as_object() {
        for (k, v) in obj {
            if !FIRST_CLASS_FIELDS.contains(&k.as_str()) {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// Require a string field.
pub fn require_string(config: &Value, key: &str) -> Result<String, AppError> {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::message(format!("Missing required field `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handle_validation() {
        assert!(validate_handle("my-ext").is_ok());
        assert!(validate_handle("-bad").is_err());
        assert!(validate_handle("bad-").is_err());
        assert!(validate_handle("bad_underscore").is_err());
    }

    #[test]
    fn strips_first_class_fields() {
        let cfg = json!({
            "type": "admin_link",
            "handle": "x",
            "uid": "u",
            "name": "Keep",
            "targeting": []
        });
        let cleaned = config_without_first_class_fields(&cfg);
        assert!(cleaned.get("type").is_none());
        assert!(cleaned.get("handle").is_none());
        assert_eq!(cleaned.get("name").and_then(|v| v.as_str()), Some("Keep"));
    }
}
