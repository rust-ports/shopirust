use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Build-related settings in `shopify.app.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildConfig {
    pub automatically_update_urls_on_dev: Option<bool>,
    pub dev_store_url: Option<String>,
    pub include_config_on_deploy: Option<bool>,
}

/// Hidden CLI-only config stored in `.shopify/project.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppHiddenConfig {
    pub dev_store_url: Option<String>,
}

/// Parsed app configuration from `shopify.app*.toml`.
///
/// Extra keys are retained in `extra` so module/config patches survive round-trips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfiguration {
    pub client_id: Option<String>,
    pub name: Option<String>,
    pub application_url: Option<String>,
    pub embedded: Option<bool>,
    pub build: Option<BuildConfig>,
    pub extension_directories: Option<Vec<String>>,
    pub web_directories: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl AppConfiguration {
    pub fn is_linked(&self) -> bool {
        self.client_id
            .as_ref()
            .map(|id| !id.is_empty())
            .unwrap_or(false)
    }

    pub fn scopes(&self) -> Vec<String> {
        self.extra
            .get("access_scopes")
            .and_then(|v| v.get("scopes"))
            .and_then(|v| v.as_str())
            .map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Normalize extension directory globs (trim trailing separators; expand trailing `*`).
    pub fn normalized_extension_directories(&self) -> Vec<String> {
        self.extension_directories
            .as_ref()
            .map(|dirs| {
                dirs.iter()
                    .map(|d| {
                        let trimmed = d.trim_end_matches(['/', '\\']);
                        if trimmed.ends_with('*') && !trimmed.ends_with("**") {
                            format!("{trimmed}*")
                        } else {
                            trimmed.to_string()
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| vec!["extensions".into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linked_app_toml() {
        let raw = r#"
client_id = "abc123"
name = "My App"
application_url = "https://example.com"
embedded = true

[access_scopes]
scopes = "write_products,read_orders"

[build]
dev_store_url = "example.myshopify.com"
"#;
        let cfg: AppConfiguration = toml::from_str(raw).unwrap();
        assert!(cfg.is_linked());
        assert_eq!(cfg.client_id.as_deref(), Some("abc123"));
        assert_eq!(
            cfg.scopes(),
            vec!["write_products".to_string(), "read_orders".to_string()]
        );
        assert_eq!(
            cfg.build.as_ref().and_then(|b| b.dev_store_url.as_deref()),
            Some("example.myshopify.com")
        );
    }

    #[test]
    fn unlinked_without_client_id() {
        let cfg: AppConfiguration = toml::from_str("name = \"x\"").unwrap();
        assert!(!cfg.is_linked());
    }
}
