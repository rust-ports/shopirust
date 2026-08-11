//! Locale configuration loader for extension deploy payloads.

use crate::error::AppError;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

/// Load `locales/*.json` for an extension directory into the deploy localization shape.
///
/// Mirrors upstream `loadLocalesConfig`: returns `{ default_locale, translations }` when
/// locale files exist, otherwise `null` (omitted by callers).
pub fn load_locales_config(directory: &Path, _name: &str) -> Result<Option<Value>, AppError> {
    let locales_dir = directory.join("locales");
    if !locales_dir.is_dir() {
        return Ok(None);
    }

    let mut translations = Map::new();
    let mut default_locale: Option<String> = None;

    let entries = fs::read_dir(&locales_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        let is_default = file_name.ends_with(".default");
        let locale = if is_default {
            file_name.trim_end_matches(".default").to_string()
        } else {
            file_name
        };

        if is_default {
            default_locale = Some(locale.clone());
        }

        let content = fs::read_to_string(&path)?;
        let parsed: Value = serde_json::from_str(&content).map_err(|e| {
            AppError::message(format!("Invalid locale JSON {}: {e}", path.display()))
        })?;
        // Upstream base64-encodes the JSON string of translations.
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            content.as_bytes(),
        );
        let _ = parsed; // validate JSON
        translations.insert(locale, Value::String(encoded));
    }

    if translations.is_empty() {
        return Ok(None);
    }

    let default_locale = default_locale.unwrap_or_else(|| {
        translations
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "en".into())
    });

    Ok(Some(json!({
        "default_locale": default_locale,
        "translations": Value::Object(translations),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn loads_default_locale() {
        let dir = tempdir().unwrap();
        let locales = dir.path().join("locales");
        fs::create_dir_all(&locales).unwrap();
        fs::write(locales.join("en.default.json"), r#"{"hello":"world"}"#).unwrap();
        fs::write(locales.join("fr.json"), r#"{"hello":"bonjour"}"#).unwrap();

        let loc = load_locales_config(dir.path(), "test").unwrap().unwrap();
        assert_eq!(
            loc.get("default_locale").and_then(|v| v.as_str()),
            Some("en")
        );
        assert!(loc.get("translations").unwrap().get("en").is_some());
        assert!(loc.get("translations").unwrap().get("fr").is_some());
    }

    #[test]
    fn missing_locales_dir_returns_none() {
        let dir = tempdir().unwrap();
        assert!(load_locales_config(dir.path(), "x").unwrap().is_none());
    }
}
