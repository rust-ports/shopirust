//! Live-reload locale payload for UI extension preview.

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use serde_json::{json, Map, Value};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub struct Localization {
    pub default_locale: String,
    pub translations: Map<String, Value>,
    pub last_updated: u64,
}

impl Localization {
    pub fn to_value(&self) -> Value {
        json!({
            "defaultLocale": self.default_locale,
            "translations": self.translations,
            "lastUpdated": self.last_updated,
        })
    }
}

/// Load `locales/*.json` for a UI extension. Empty dir → `None` with empty status.
pub fn get_localization(
    extension: &ExtensionInstance,
    current: Option<&Value>,
) -> Result<(Option<Value>, String), AppError> {
    let locales_dir = extension.directory.join("locales");
    if !locales_dir.is_dir() {
        return Ok((None, String::new()));
    }

    let mut files: Vec<_> = fs::read_dir(&locales_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    if files.is_empty() {
        return Ok((None, String::new()));
    }
    files.sort();

    let mut localization = current
        .cloned()
        .and_then(parse_existing)
        .unwrap_or_else(|| Localization {
            default_locale: "en".into(),
            translations: Map::new(),
            last_updated: 0,
        });

    for path in &files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("en")
            .to_string();
        let (locale, is_default) = if let Some(stripped) = stem.strip_suffix(".default") {
            (stripped.to_string(), true)
        } else {
            (stem, false)
        };
        if is_default {
            localization.default_locale = locale.clone();
        }
        let raw = fs::read_to_string(path).map_err(|e| {
            AppError::message(format!("Error loading locale file {}: {e}", path.display()))
        })?;
        let parsed: Value = serde_json::from_str(&raw).map_err(|e| {
            AppError::message(format!(
                "Invalid JSON in locale file {}: {e}",
                path.display()
            ))
        })?;
        localization.translations.insert(locale, parsed);
    }

    localization.last_updated = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok((Some(localization.to_value()), "success".into()))
}

fn parse_existing(value: Value) -> Option<Localization> {
    let obj = value.as_object()?;
    Some(Localization {
        default_locale: obj
            .get("defaultLocale")
            .and_then(|v| v.as_str())
            .unwrap_or("en")
            .to_string(),
        translations: obj
            .get("translations")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default(),
        last_updated: obj.get("lastUpdated").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::tempdir;

    fn ui_ext(dir: &std::path::Path) -> ExtensionInstance {
        let spec = create_extension_specification("ui_extension").unwrap();
        ExtensionInstance::new(
            "ui",
            dir.to_path_buf(),
            dir.join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        )
    }

    #[test]
    fn empty_locales_returns_none() {
        let dir = tempdir().unwrap();
        let ext = ui_ext(dir.path());
        let (loc, status) = get_localization(&ext, None).unwrap();
        assert!(loc.is_none());
        assert_eq!(status, "");
    }

    #[test]
    fn loads_default_and_named_locales() {
        let dir = tempdir().unwrap();
        let locales = dir.path().join("locales");
        fs::create_dir_all(&locales).unwrap();
        fs::write(locales.join("en.default.json"), r#"{"hello":"Hello"}"#).unwrap();
        fs::write(locales.join("fr.json"), r#"{"hello":"Bonjour"}"#).unwrap();
        let ext = ui_ext(dir.path());
        let (loc, status) = get_localization(&ext, None).unwrap();
        assert_eq!(status, "success");
        let loc = loc.unwrap();
        assert_eq!(loc["defaultLocale"], "en");
        assert_eq!(loc["translations"]["en"]["hello"], "Hello");
        assert_eq!(loc["translations"]["fr"]["hello"], "Bonjour");
        assert!(loc["lastUpdated"].as_u64().unwrap() > 0);
    }

    #[test]
    fn invalid_json_errors() {
        let dir = tempdir().unwrap();
        let locales = dir.path().join("locales");
        fs::create_dir_all(&locales).unwrap();
        let mut f = fs::File::create(locales.join("en.json")).unwrap();
        f.write_all(b"{not json").unwrap();
        let ext = ui_ext(dir.path());
        let err = get_localization(&ext, None).unwrap_err();
        assert!(err.to_string().contains("Invalid JSON"));
    }
}
