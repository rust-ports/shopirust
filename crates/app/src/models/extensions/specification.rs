use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionExperience {
    Extension,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionFeature {
    UiPreview,
    Function,
    Theme,
    CartUrl,
    Esbuild,
    SingleJsEntryPath,
    Localization,
    GeneratesSourceMaps,
}

/// Declarative extension specification (local + remote-aware fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionSpecification {
    pub identifier: String,
    pub external_identifier: String,
    pub external_name: String,
    pub partners_web_identifier: String,
    pub surface: String,
    pub experience: ExtensionExperience,
    pub registration_limit: usize,
    pub additional_identifiers: Vec<String>,
    pub group: Option<String>,
    pub features: Vec<ExtensionFeature>,
}

impl ExtensionSpecification {
    pub fn is_app_config(&self) -> bool {
        matches!(self.experience, ExtensionExperience::Configuration)
    }

    pub fn matches_type(&self, type_name: &str) -> bool {
        self.identifier == type_name
            || self.external_identifier == type_name
            || self.additional_identifiers.iter().any(|id| id == type_name)
    }
}

/// Build a specification from a small registry of known types.
pub fn create_extension_specification(identifier: &str) -> Option<ExtensionSpecification> {
    crate::models::extensions::specifications::lookup(identifier)
}

/// Parse a minimal TOML/JSON extension config object.
pub fn parse_base_config(value: &Value) -> Result<HashMap<String, Value>, String> {
    value
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .ok_or_else(|| "extension config must be a table/object".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_spec_lookup() {
        let spec = create_extension_specification("theme").expect("theme");
        assert_eq!(spec.identifier, "theme");
        assert!(spec.features.contains(&ExtensionFeature::Theme));
        assert!(!spec.is_app_config());
    }

    #[test]
    fn ui_extension_matches_aliases() {
        let spec = create_extension_specification("ui_extension").unwrap();
        assert!(spec.matches_type("ui_extension"));
        assert!(spec.matches_type("checkout_ui_extension"));
    }
}
