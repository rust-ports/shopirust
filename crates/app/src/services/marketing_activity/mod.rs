//! Marketing activity import helpers (upstream `services/marketing_activity/`).

use crate::error::AppError;
use crate::models::extensions::schemas::MAX_EXTENSION_HANDLE_LENGTH;
use crate::services::generate::slugify;
use crate::services::import_extensions::ExtensionRegistration;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

fn platform_channel_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("facebook", "social"),
        ("instagram", "social"),
        ("google", "search"),
        ("pinterest", "social"),
        ("bing", "search"),
        ("email", "email"),
        ("snapchat", "social"),
        ("sms", "sms"),
        ("verizon_media", "display"),
        ("ebay", "marketplace"),
        ("tiktok", "social"),
        ("flow", "email"),
    ])
}

fn platform_domain_map() -> HashMap<&'static str, Option<&'static str>> {
    HashMap::from([
        ("facebook", Some("facebook.com")),
        ("instagram", Some("instagram.com")),
        ("google", Some("google.com")),
        ("pinterest", Some("pinterest.com")),
        ("bing", Some("bing.com")),
        ("snapchat", Some("snapchat.com")),
        ("verizon_media", None),
        ("email", None),
        ("sms", None),
        ("ebay", Some("ebay.com")),
        ("tiktok", Some("tiktok.com")),
        ("flow", None),
    ])
}

fn get_url_path(url: &str) -> Result<String, AppError> {
    let parsed = url::Url::parse(url).map_err(|e| AppError::message(e.to_string()))?;
    let mut path = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        path.push('?');
        path.push_str(query);
    }
    if let Some(fragment) = parsed.fragment() {
        path.push('#');
        path.push_str(fragment);
    }
    Ok(path)
}

fn truncated_handle(title: &str) -> String {
    let truncated: String = title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect();
    slugify(&truncated)
}

/// Convert a dashboard marketing activity registration into local TOML JSON.
pub fn build_extension_config(extension: &ExtensionRegistration) -> Result<Value, AppError> {
    let version_config = extension
        .active_version
        .as_ref()
        .or(extension.draft_version.as_ref())
        .and_then(|v| v.config.as_deref())
        .ok_or_else(|| AppError::message("No config found for extension"))?;

    let config: Value = serde_json::from_str(version_config)?;
    let platform = config
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let channels = platform_channel_map();
    let domains = platform_domain_map();

    let fields = config
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|field| {
            let mut obj = match field {
                Value::Object(m) => m,
                other => {
                    let mut m = Map::new();
                    m.insert("value".into(), other);
                    m
                }
            };
            obj.remove("id");
            Value::Object(obj)
        })
        .collect::<Vec<_>>();

    let api_path = config
        .get("app_api_url")
        .and_then(|v| v.as_str())
        .map(get_url_path)
        .transpose()?
        .unwrap_or_default();

    Ok(json!({
        "extensions": [{
            "type": "marketing_activity",
            "name": extension.title,
            "handle": truncated_handle(&extension.title),
            "title": config.get("title"),
            "description": config.get("description"),
            "api_path": api_path,
            "tactic": config.get("tactic"),
            "marketing_channel": channels.get(platform).copied().unwrap_or(""),
            "referring_domain": domains.get(platform).copied().flatten().unwrap_or(""),
            "is_automation": config.get("is_automation"),
            "use_external_editor": config.get("use_external_editor"),
            "preview_data": config.get("preview_data"),
            "fields": fields,
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_extensions::ExtensionVersion;

    fn sample_config() -> Value {
        json!({
            "title": "test mae",
            "description": "test mae description",
            "app_api_url": "https://google.es/api/v1",
            "tactic": "ad",
            "platform": "facebook",
            "is_automation": false,
            "preview_data": [{"label": "test label", "value": "test value"}],
            "fields": [{
                "id": "123",
                "ui_type": "text-single-line",
                "name": "test_field",
                "label": "test field",
                "help_text": "help text",
                "required": false,
                "min_length": 1,
                "max_length": 50,
                "placeholder": "placeholder"
            }]
        })
    }

    #[test]
    fn converts_dashboard_config() {
        let extension = ExtensionRegistration {
            uuid: "ad9947a9-bc0b-4855-82da-008aefbc1c71".into(),
            title: "mae @ test! 123".into(),
            type_name: "marketing_activity_extension".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(sample_config().to_string()),
                context: None,
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("mae-test-123")
        );
        assert_eq!(
            got.pointer("/extensions/0/api_path")
                .and_then(|v| v.as_str()),
            Some("/api/v1")
        );
        assert_eq!(
            got.pointer("/extensions/0/marketing_channel")
                .and_then(|v| v.as_str()),
            Some("social")
        );
        assert_eq!(
            got.pointer("/extensions/0/referring_domain")
                .and_then(|v| v.as_str()),
            Some("facebook.com")
        );
        assert!(got.pointer("/extensions/0/fields/0/id").is_none());
    }

    #[test]
    fn truncates_handle_and_unknown_platform() {
        let mut cfg = sample_config();
        cfg.as_object_mut()
            .unwrap()
            .insert("platform".into(), json!("not-a-platform"));
        let extension = ExtensionRegistration {
            uuid: "u".into(),
            title: "mae @ test! 1234555555555444444777777888888812345555555554444447777778888888"
                .into(),
            type_name: "marketing_activity_extension".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(cfg.to_string()),
                context: None,
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("mae-test-12345555555554444447777778888888123455")
        );
        assert_eq!(
            got.pointer("/extensions/0/marketing_channel")
                .and_then(|v| v.as_str()),
            Some("")
        );
        assert_eq!(
            got.pointer("/extensions/0/referring_domain")
                .and_then(|v| v.as_str()),
            Some("")
        );
    }
}
