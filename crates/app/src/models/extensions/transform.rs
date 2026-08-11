//! App-config path transforms (mirrors upstream `appConfigTransform`).

use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Path-map transform: remote_key → local dotted path (forward), or reverse.
pub type TransformationConfig = HashMap<&'static str, &'static str>;

/// Get a value at a dotted path (`access_scopes.scopes`).
pub fn get_path_value<'a>(content: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = content;
    for part in path.split('.') {
        cur = cur.as_object()?.get(part)?;
    }
    Some(cur)
}

/// Set a value at a dotted path, creating intermediate objects.
pub fn set_path_value(content: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    if !content.is_object() {
        *content = Value::Object(Map::new());
    }
    let mut cur = content.as_object_mut().unwrap();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            cur.insert((*part).to_string(), value);
            return;
        }
        let entry = cur
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cur = entry.as_object_mut().unwrap();
    }
}

/// Apply a path-map transform. `reverse=false`: local → remote (map value is local path).
pub fn app_config_transform(
    content: &Value,
    config: &TransformationConfig,
    reverse: bool,
) -> Value {
    let mut transformed = Value::Object(Map::new());
    for (mapped_path, object_path) in config {
        let (origin, target) = if reverse {
            (*mapped_path, *object_path)
        } else {
            (*object_path, *mapped_path)
        };
        if let Some(source) = get_path_value(content, origin) {
            set_path_value(&mut transformed, target, source.clone());
        }
    }
    transformed
}

/// Prepend application URL to a relative URI (`/path` → `https://app.example/path`).
pub fn prepend_application_url(uri: &str, application_url: &str) -> String {
    let uri = uri.trim_end_matches('/');
    if uri.starts_with('/') {
        let base = application_url.trim_end_matches('/');
        format!("{base}{uri}")
    } else {
        uri.to_string()
    }
}

/// Built-in path maps for config specifications.
pub fn branding_transform() -> TransformationConfig {
    HashMap::from([("name", "name"), ("app_handle", "handle")])
}

pub fn app_access_transform() -> TransformationConfig {
    HashMap::from([
        ("access", "access"),
        ("scopes", "access_scopes.scopes"),
        ("required_scopes", "access_scopes.required_scopes"),
        ("optional_scopes", "access_scopes.optional_scopes"),
        (
            "use_legacy_install_flow",
            "access_scopes.use_legacy_install_flow",
        ),
        ("redirect_url_allowlist", "auth.redirect_urls"),
    ])
}

pub fn app_home_transform() -> TransformationConfig {
    HashMap::from([
        ("app_url", "application_url"),
        ("embedded", "embedded"),
        ("preferences_url", "app_preferences.url"),
    ])
}

pub fn point_of_sale_transform() -> TransformationConfig {
    HashMap::from([("embedded", "pos.embedded")])
}

/// Custom forward for `app_proxy`.
pub fn transform_app_proxy_forward(local: &Value, application_url: &str) -> Value {
    let Some(proxy) = local.get("app_proxy") else {
        return json!({});
    };
    let url = proxy
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    json!({
        "url": prepend_application_url(url, application_url),
        "subpath": proxy.get("subpath").cloned().unwrap_or(Value::Null),
        "prefix": proxy.get("prefix").cloned().unwrap_or(Value::Null),
    })
}

pub fn transform_app_proxy_reverse(remote: &Value) -> Value {
    let url = remote
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim_end_matches('/');
    json!({
        "app_proxy": {
            "url": url,
            "subpath": remote.get("subpath").cloned().unwrap_or(Value::Null),
            "prefix": remote.get("prefix").cloned().unwrap_or(Value::Null),
        }
    })
}

/// Webhooks module only ships `api_version`.
pub fn transform_webhooks_forward(local: &Value) -> Value {
    let api_version = get_path_value(local, "webhooks.api_version");
    match api_version {
        Some(v) => json!({ "api_version": v }),
        None => json!({}),
    }
}

pub fn transform_webhooks_reverse(remote: &Value) -> Value {
    match remote.get("api_version") {
        Some(v) => json!({ "webhooks": { "api_version": v } }),
        None => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branding_forward_maps_handle() {
        let local = json!({ "name": "My App", "handle": "my-app" });
        let remote = app_config_transform(&local, &branding_transform(), false);
        assert_eq!(remote.get("name").and_then(|v| v.as_str()), Some("My App"));
        assert_eq!(
            remote.get("app_handle").and_then(|v| v.as_str()),
            Some("my-app")
        );
    }

    #[test]
    fn app_access_nested_paths() {
        let local = json!({
            "access_scopes": { "scopes": "read_products,write_products" },
            "auth": { "redirect_urls": ["https://example.com/cb"] }
        });
        let remote = app_config_transform(&local, &app_access_transform(), false);
        assert_eq!(
            remote.get("scopes").and_then(|v| v.as_str()),
            Some("read_products,write_products")
        );
        assert_eq!(
            remote
                .get("redirect_url_allowlist")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(1)
        );
    }

    #[test]
    fn prepend_relative_url() {
        assert_eq!(
            prepend_application_url("/proxy", "https://app.example.com"),
            "https://app.example.com/proxy"
        );
        assert_eq!(
            prepend_application_url("https://other.com/x", "https://app.example.com"),
            "https://other.com/x"
        );
    }
}
