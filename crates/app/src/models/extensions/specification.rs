use crate::error::AppError;
use crate::models::extensions::deploy::{
    build_deploy_config, patch_with_app_dev_urls, transform_local_to_remote,
    transform_remote_to_local, validate_configuration,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UidStrategy {
    Single,
    Dynamic,
    #[default]
    Uuid,
}

/// Declarative extension specification (local + remote-aware fields + behavior hooks).
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
    pub uid_strategy: UidStrategy,
    pub graph_ql_type: Option<String>,
    pub dependency: Option<String>,
    /// Remote App Management `validationSchema.jsonSchema`, if attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
}

impl ExtensionSpecification {
    pub fn is_app_config(&self) -> bool {
        matches!(self.experience, ExtensionExperience::Configuration)
    }

    pub fn matches_type(&self, type_name: &str) -> bool {
        self.identifier == type_name
            || self.external_identifier == type_name
            || self.additional_identifiers.iter().any(|id| id == type_name)
            || self
                .graph_ql_type
                .as_deref()
                .is_some_and(|g| g == type_name)
    }

    pub async fn deploy_config(
        &self,
        configuration: &HashMap<String, Value>,
        directory: &Path,
        ctx: &crate::models::extensions::deploy::DeployConfigContext,
    ) -> Result<Option<Value>, AppError> {
        build_deploy_config(self, configuration, directory, ctx).await
    }

    pub fn transform_local_to_remote(
        &self,
        local: &Value,
        app_configuration: Option<&Value>,
    ) -> Value {
        transform_local_to_remote(self, local, app_configuration)
    }

    pub fn transform_remote_to_local(&self, remote: &Value) -> Value {
        transform_remote_to_local(self, remote)
    }

    pub fn validate(
        &self,
        configuration: &HashMap<String, Value>,
        directory: &Path,
    ) -> Result<(), AppError> {
        validate_configuration(self, configuration, directory)
    }

    pub fn patch_with_app_dev_urls(
        &self,
        configuration: &mut HashMap<String, Value>,
        urls: &crate::models::extensions::deploy::AppDevUrls,
    ) {
        patch_with_app_dev_urls(self, configuration, urls)
    }
}

/// Build a specification from the local registry.
pub fn create_extension_specification(identifier: &str) -> Option<ExtensionSpecification> {
    crate::models::extensions::specifications::lookup(identifier)
}

/// Copy AM `validationSchema.jsonSchema` onto matching local specs.
pub fn apply_remote_json_schemas(
    specs: &mut [ExtensionSpecification],
    remote: &[cli_api::RemoteSpecification],
) {
    for spec in specs {
        if let Some(remote) = remote.iter().find(|r| {
            r.identifier == spec.identifier
                || spec
                    .additional_identifiers
                    .iter()
                    .any(|a| a == &r.identifier)
        }) {
            if let Some(schema) = &remote.validation_schema {
                spec.json_schema = Some(schema.clone());
            }
        }
    }
}

/// Parse a minimal TOML/JSON extension config object.
pub fn parse_base_config(value: &Value) -> Result<HashMap<String, Value>, String> {
    value
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .ok_or_else(|| "extension config must be a table/object".into())
}

pub use crate::models::extensions::deploy::{AppDevUrls, AppProxyUrls, DeployConfigContext};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_spec_lookup() {
        let spec = create_extension_specification("theme").expect("theme");
        assert_eq!(spec.identifier, "theme");
        assert!(spec.features.contains(&ExtensionFeature::Theme));
        assert!(!spec.is_app_config());
        assert_eq!(spec.partners_web_identifier.as_str(), "theme_app_extension");
    }

    #[test]
    fn ui_extension_and_checkout_ui_are_distinct() {
        let ui = create_extension_specification("ui_extension").unwrap();
        let checkout = create_extension_specification("checkout_ui_extension").unwrap();
        assert!(ui.matches_type("ui_extension"));
        assert!(!ui.matches_type("checkout_ui_extension"));
        assert_eq!(checkout.identifier, "checkout_ui_extension");
        assert!(checkout.features.contains(&ExtensionFeature::UiPreview));
    }

    #[test]
    fn config_specs_use_single_uid() {
        let branding = create_extension_specification("branding").unwrap();
        assert_eq!(branding.uid_strategy, UidStrategy::Single);
        assert!(branding.is_app_config());
    }

    #[test]
    fn webhook_subscription_is_dynamic() {
        let spec = create_extension_specification("webhook_subscription").unwrap();
        assert_eq!(spec.uid_strategy, UidStrategy::Dynamic);
    }

    #[test]
    fn apply_remote_json_schemas_copies_contract() {
        let mut specs = vec![create_extension_specification("ui_extension").unwrap()];
        let remote = vec![cli_api::RemoteSpecification {
            identifier: "ui_extension".into(),
            name: "UI".into(),
            experience: "extension".into(),
            options: None,
            validation_schema: Some(serde_json::json!({
                "type": "object",
                "required": ["name"]
            })),
        }];
        apply_remote_json_schemas(&mut specs, &remote);
        assert!(specs[0].json_schema.is_some());
        assert_eq!(
            specs[0].json_schema.as_ref().unwrap()["required"][0],
            "name"
        );
    }
}
