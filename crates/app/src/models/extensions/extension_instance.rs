use crate::error::AppError;
use crate::models::extensions::deploy::DeployConfigContext;
use crate::models::extensions::specification::{
    ExtensionFeature, ExtensionSpecification, UidStrategy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single function targeting entry from extension TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionTargeting {
    pub target: String,
    pub input_query: Option<String>,
    pub export: Option<String>,
}

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

    /// Upstream `isPreviewable` — UI extensions served by the extension preview server.
    pub fn is_previewable(&self) -> bool {
        self.is_ui_extension()
    }

    /// UUID-strategy extensions (and app_access) can be pushed as Partners drafts.
    pub fn is_uuid_strategy_extension(&self) -> bool {
        matches!(self.specification.uid_strategy, UidStrategy::Uuid)
    }

    /// Upstream `draftableExtensions` membership.
    pub fn is_draftable(&self) -> bool {
        self.is_uuid_strategy_extension() || self.specification.identifier == "app_access"
    }

    pub fn is_app_config_extension(&self) -> bool {
        self.specification.is_app_config()
    }

    pub fn has_esbuild_feature(&self) -> bool {
        self.specification
            .features
            .contains(&ExtensionFeature::Esbuild)
    }

    /// GraphQL `context` for draft/create (first targeting target).
    pub fn context_value(&self) -> Option<String> {
        self.extension_point_targets().into_iter().next()
    }

    pub fn surface(&self) -> &str {
        &self.specification.surface
    }

    pub fn external_type(&self) -> &str {
        &self.specification.external_identifier
    }

    /// Ensure `dev_uuid` is set (`dev-{uid}` or a fresh uuid). Returns the uuid.
    pub fn ensure_dev_uuid(&mut self) -> &str {
        if self.dev_uuid.is_none() {
            let base = self
                .uid
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            self.dev_uuid = Some(format!("dev-{base}"));
        }
        self.dev_uuid.as_deref().unwrap()
    }

    pub fn should_fetch_cart_url(&self) -> bool {
        self.specification
            .features
            .contains(&ExtensionFeature::CartUrl)
    }

    /// JS output filename for UI extensions (handle.js).
    pub fn output_file_name(&self) -> String {
        format!("{}.js", self.handle)
    }

    /// Absolute output path inside a bundle directory (dev-bundle or deploy bundle).
    pub fn get_output_path_for_directory(&self, bundle_directory: &Path) -> PathBuf {
        if let Some(ref out) = self.output_path {
            if out.is_absolute() {
                return out.clone();
            }
            return bundle_directory.join(out);
        }
        if self.is_function_extension() {
            return bundle_directory
                .join(&self.handle)
                .join(self.output_relative_path());
        }
        bundle_directory
            .join(&self.handle)
            .join(self.output_file_name())
    }

    /// Whether this extension declares the given extension-point target.
    pub fn has_extension_point_target(&self, target: &str) -> bool {
        if self.type_name() == "checkout_post_purchase" {
            return target == "purchase.post.render";
        }
        self.extension_point_targets().iter().any(|t| t == target)
    }

    /// Targets from `extension_points` or `targeting` config arrays.
    pub fn extension_point_targets(&self) -> Vec<String> {
        let points = self
            .configuration
            .get("extension_points")
            .or_else(|| self.configuration.get("targeting"));
        let Some(Value::Array(items)) = points else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|item| {
                item.get("target")
                    .and_then(|t| t.as_str())
                    .map(str::to_string)
            })
            .collect()
    }

    /// Files to watch for hot-reload (extension directory contents, excluding common noise).
    pub fn watched_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.configuration_path.clone()];
        if let Some(ref entry) = self.entry_path {
            files.push(entry.clone());
        }
        collect_watch_files(&self.directory, &mut files);
        files.sort();
        files.dedup();
        files
    }

    /// Upstream `bundleURL`-style relative output path for the extension.
    pub fn bundle_url(&self) -> String {
        if let Some(ref out) = self.output_path {
            return out.to_string_lossy().replace('\\', "/");
        }
        format!("dist/{}/{}", self.specification.identifier, self.handle)
    }

    pub fn type_name(&self) -> &str {
        self.configuration
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.specification.identifier)
    }

    /// Display name from TOML `name`, falling back to handle.
    pub fn name(&self) -> String {
        self.configuration
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.handle.clone())
    }

    pub fn api_version(&self) -> Option<&str> {
        self.configuration
            .get("api_version")
            .and_then(|v| v.as_str())
    }

    /// True when the function entrypoint looks like JavaScript/TypeScript.
    pub fn is_javascript(&self) -> bool {
        if let Some(ref entry) = self.entry_path {
            if matches!(
                entry
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .as_deref(),
                Some("js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs")
            ) {
                return true;
            }
        }
        self.directory.join("package.json").exists()
            && (self.directory.join("src/index.js").exists()
                || self.directory.join("src/index.ts").exists()
                || self.directory.join("src/index.jsx").exists()
                || self.directory.join("src/index.tsx").exists())
    }

    pub fn build_command(&self) -> Option<String> {
        self.configuration
            .get("build")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
    }

    pub fn typegen_command(&self) -> Option<String> {
        self.configuration
            .get("build")
            .and_then(|v| v.get("typegen_command"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
    }

    /// Relative wasm path from `[build].path`, else `dist/index.wasm`.
    pub fn output_relative_path(&self) -> String {
        self.configuration
            .get("build")
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "dist/index.wasm".into())
    }

    /// Absolute wasm output path for this function extension.
    pub fn function_output_path(&self) -> PathBuf {
        if let Some(ref out) = self.output_path {
            return out.clone();
        }
        self.directory.join(self.output_relative_path())
    }

    /// Whether wasm-opt should run (defaults to true when unset).
    pub fn wasm_opt_enabled(&self) -> bool {
        self.configuration
            .get("build")
            .and_then(|v| v.get("wasm_opt"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub fn targeting(&self) -> Vec<FunctionTargeting> {
        let Some(Value::Array(items)) = self.configuration.get("targeting") else {
            return vec![];
        };
        items
            .iter()
            .filter_map(|item| {
                let target = item.get("target")?.as_str()?.to_string();
                Some(FunctionTargeting {
                    target,
                    input_query: item
                        .get("input_query")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    export: item
                        .get("export")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                })
            })
            .collect()
    }

    /// Find `node_modules/@shopify/shopify_function/package.json` walking up from the extension.
    pub fn shopify_function_package_json(&self) -> Option<PathBuf> {
        find_up(
            &self.directory,
            Path::new("node_modules/@shopify/shopify_function/package.json"),
        )
    }

    /// Validate this extension's configuration against its specification.
    pub fn validate(&self) -> Result<(), AppError> {
        self.specification
            .validate(&self.configuration, &self.directory)
    }

    /// Build the platform deploy payload for this extension.
    pub async fn deploy_config(
        &self,
        ctx: &DeployConfigContext,
    ) -> Result<Option<Value>, AppError> {
        self.specification
            .deploy_config(&self.configuration, &self.directory, ctx)
            .await
    }

    /// Transform local app-config content using this specification.
    pub fn transform_local_to_remote(&self, app_configuration: Option<&Value>) -> Value {
        let local = Value::Object(
            self.configuration
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        self.specification
            .transform_local_to_remote(&local, app_configuration)
    }
}

fn find_up(start: &Path, relative: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        let candidate = current.join(relative);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

fn collect_watch_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "node_modules" || name == ".git" || name == "dist" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_watch_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
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
        assert!(ext.is_draftable());
        assert!(ext.is_uuid_strategy_extension());
        assert!(!ext.is_previewable());
        assert!(!ext.has_esbuild_feature());
        assert!(!ext.is_app_config_extension());
    }

    #[test]
    fn ui_extension_is_previewable_and_draftable() {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert(
            "targeting".into(),
            serde_json::json!([{ "target": "purchase.checkout.block.render" }]),
        );
        let ext = ExtensionInstance::new(
            "checkout-ui",
            PathBuf::from("extensions/checkout-ui"),
            PathBuf::from("extensions/checkout-ui/shopify.extension.toml"),
            cfg,
            spec,
        );
        assert!(ext.is_previewable());
        assert!(ext.is_draftable());
        assert_eq!(
            ext.context_value().as_deref(),
            Some("purchase.checkout.block.render")
        );
    }

    #[test]
    fn function_output_path_defaults() {
        let spec = create_extension_specification("function").unwrap();
        let ext = ExtensionInstance::new(
            "discount",
            PathBuf::from("extensions/discount"),
            PathBuf::from("extensions/discount/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert!(ext.is_function_extension());
        assert!(ext.is_draftable());
        assert_eq!(
            ext.function_output_path(),
            PathBuf::from("extensions/discount/dist/index.wasm")
        );
        assert_eq!(ext.output_file_name(), "discount.js");
    }

    #[test]
    fn watched_files_includes_toml() {
        let spec = create_extension_specification("theme").unwrap();
        let toml = PathBuf::from("extensions/my-theme-ext/shopify.extension.toml");
        let ext = ExtensionInstance::new(
            "my-theme-ext",
            PathBuf::from("extensions/my-theme-ext"),
            toml.clone(),
            HashMap::new(),
            spec,
        );
        let files = ext.watched_files();
        assert!(files.contains(&toml));
    }

    #[test]
    fn ensure_dev_uuid_is_stable() {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut ext = ExtensionInstance::new(
            "ui",
            PathBuf::from("extensions/ui"),
            PathBuf::from("extensions/ui/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        ext.uid = Some("fixed-uid".into());
        let first = ext.ensure_dev_uuid().to_string();
        let second = ext.ensure_dev_uuid().to_string();
        assert_eq!(first, "dev-fixed-uid");
        assert_eq!(first, second);
    }

    #[test]
    fn config_module_is_not_draftable() {
        let spec = create_extension_specification("branding").unwrap();
        let ext = ExtensionInstance::new(
            "branding",
            PathBuf::from("."),
            PathBuf::from("shopify.app.toml"),
            HashMap::new(),
            spec,
        );
        assert!(ext.is_app_config_extension());
        assert!(!ext.is_draftable());
        assert!(!ext.is_previewable());
    }

    #[test]
    fn checkout_ui_has_esbuild_and_cart_url() {
        let spec = create_extension_specification("checkout_ui_extension").unwrap();
        let ext = ExtensionInstance::new(
            "checkout-ui",
            PathBuf::from("extensions/checkout-ui"),
            PathBuf::from("extensions/checkout-ui/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert!(ext.has_esbuild_feature());
        assert!(ext.is_previewable());
        assert!(ext.is_draftable());
    }

    #[test]
    fn post_purchase_is_previewable() {
        let spec = create_extension_specification("checkout_post_purchase").unwrap();
        let ext = ExtensionInstance::new(
            "post-purchase",
            PathBuf::from("extensions/pp"),
            PathBuf::from("extensions/pp/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert!(ext.is_previewable());
        assert_eq!(ext.output_file_name(), "post-purchase.js");
    }

    #[test]
    fn payments_is_uuid_strategy() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let ext = ExtensionInstance::new(
            "offsite",
            PathBuf::from("extensions/offsite"),
            PathBuf::from("extensions/offsite/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert!(ext.is_uuid_strategy_extension());
        assert!(!ext.is_function_extension());
    }
}
