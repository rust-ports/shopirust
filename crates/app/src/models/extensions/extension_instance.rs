use crate::models::extensions::specification::{ExtensionFeature, ExtensionSpecification};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// A loaded extension instance (local filesystem + typed config).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInstance {
    pub handle: String,
    pub directory: PathBuf,
    pub configuration_path: PathBuf,
    pub configuration: HashMap<String, Value>,
    pub specification: ExtensionSpecification,
    pub entry_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub uid: Option<String>,
    pub dev_uuid: Option<String>,
}

impl ExtensionInstance {
    pub fn new(
        handle: impl Into<String>,
        directory: PathBuf,
        configuration_path: PathBuf,
        configuration: HashMap<String, Value>,
        specification: ExtensionSpecification,
    ) -> Self {
        Self {
            handle: handle.into(),
            directory,
            configuration_path,
            configuration,
            specification,
            entry_path: None,
            output_path: None,
            uid: None,
            dev_uuid: None,
        }
    }

    pub fn local_identifier(&self) -> &str {
        &self.handle
    }

    pub fn graph_ql_type(&self) -> &str {
        &self.specification.identifier
    }

    pub fn is_theme_extension(&self) -> bool {
        self.specification
            .features
            .contains(&ExtensionFeature::Theme)
    }

    pub fn is_function_extension(&self) -> bool {
        self.specification
            .features
            .contains(&ExtensionFeature::Function)
    }

    pub fn is_ui_extension(&self) -> bool {
        self.specification
            .features
            .contains(&ExtensionFeature::UiPreview)
    }

    /// Upstream `bundleURL`-style relative output path for the extension.
    pub fn bundle_url(&self) -> String {
        if let Some(ref out) = self.output_path {
            return out.to_string_lossy().replace('\\', "/");
        }
        format!(
            "dist/{}/{}",
            self.specification.identifier, self.handle
        )
    }

    pub fn type_name(&self) -> &str {
        self.configuration
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.specification.identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;

    #[test]
    fn bundle_url_defaults() {
        let spec = create_extension_specification("theme").unwrap();
        let ext = ExtensionInstance::new(
            "my-theme-ext",
            PathBuf::from("extensions/my-theme-ext"),
            PathBuf::from("extensions/my-theme-ext/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert_eq!(ext.bundle_url(), "dist/theme/my-theme-ext");
        assert!(ext.is_theme_extension());
    }
}
