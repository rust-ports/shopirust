//! UI extension specification + deploy config (upstream `ui_extension.ts`).

use crate::error::AppError;
use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification, UidStrategy,
};
use crate::utilities::locales::load_locales_config;
use serde_json::{json, Value};
use std::path::Path;

pub fn ui_extension_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "ui_extension".into(),
        external_identifier: "ui_extension_external".into(),
        external_name: "UI extension".into(),
        partners_web_identifier: "ui_extension".into(),
        surface: "checkout".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: vec![],
        group: Some("Checkout".into()),
        features: vec![
            ExtensionFeature::UiPreview,
            ExtensionFeature::Esbuild,
            ExtensionFeature::GeneratesSourceMaps,
            ExtensionFeature::CartUrl,
            ExtensionFeature::SingleJsEntryPath,
            ExtensionFeature::Localization,
        ],
        uid_strategy: UidStrategy::Uuid,
        graph_ql_type: None,
        dependency: Some("@shopify/checkout-ui-extensions".into()),
    }
}

/// Checkout UI target that should render a dedicated conditions bundle.
pub fn get_should_render_target(target: &str) -> bool {
    target.contains("::should-render") || target.ends_with(".should-render")
}

pub fn validate_ui_extension(config: &Value, directory: &Path) -> Result<(), AppError> {
    crate::models::extensions::schemas::require_string(config, "name")?;
    if config.get("targeting").is_none() && config.get("extension_points").is_none() {
        return Err(AppError::message(
            "No extension targets defined, add a `targeting` field to your configuration",
        ));
    }
    validate_ui_modules(config, directory)
}

pub fn validate_ui_modules(config: &Value, directory: &Path) -> Result<(), AppError> {
    let points = config
        .get("targeting")
        .or_else(|| config.get("extension_points"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut targets = Vec::new();
    for point in &points {
        let target = point
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if targets.contains(&target.to_string()) {
            return Err(AppError::message(format!(
                "Duplicate extension target `{target}`"
            )));
        }
        targets.push(target.to_string());
        if let Some(module) = point.get("module").and_then(|v| v.as_str()) {
            let path = directory.join(module);
            if !path.is_file() {
                return Err(AppError::message(format!("Couldn't find {module}")));
            }
        }
    }
    Ok(())
}

pub async fn deploy_ui_extension(config: &Value, directory: &Path) -> Result<Option<Value>, AppError> {
    let handle = config
        .get("handle")
        .and_then(|v| v.as_str())
        .unwrap_or("extension");
    let points = config
        .get("targeting")
        .or_else(|| config.get("extension_points"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut extension_points = Vec::new();
    for targeting in points {
        let module = targeting
            .get("module")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let target = targeting.get("target").cloned().unwrap_or(Value::Null);
        let mut build_manifest = json!({
            "assets": {
                "main": {
                    "filepath": format!("{handle}.js"),
                    "module": module,
                }
            }
        });
        if let Some(sr) = targeting
            .pointer("/should_render/module")
            .and_then(|v| v.as_str())
        {
            build_manifest["assets"]["should_render"] = json!({
                "filepath": format!("{handle}-conditions.js"),
                "module": sr,
            });
        }
        extension_points.push(json!({
            "target": target,
            "module": module,
            "metafields": targeting.get("metafields")
                .cloned()
                .or_else(|| config.get("metafields").cloned())
                .unwrap_or_else(|| json!([])),
            "default_placement_reference": targeting.get("default_placement"),
            "urls": targeting.get("urls").cloned().unwrap_or_else(|| json!({})),
            "capabilities": targeting.get("capabilities"),
            "preloads": targeting.get("preloads").cloned().unwrap_or_else(|| json!({})),
            "build_manifest": build_manifest,
            "tools": targeting.get("tools"),
            "instructions": targeting.get("instructions"),
            "intents": targeting.get("intents"),
            "assets": targeting.get("assets"),
        }));
    }

    let localization = load_locales_config(
        directory,
        config
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("ui_extension"),
    )?;

    Ok(Some(json!({
        "api_version": config.get("api_version"),
        "extension_points": extension_points,
        "capabilities": config.get("capabilities"),
        "supported_features": config.get("supported_features"),
        "name": config.get("name"),
        "description": config.get("description"),
        "settings": config.get("settings"),
        "localization": localization,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::deploy::{build_deploy_config, validate_configuration};
    use crate::models::extensions::specification::create_extension_specification;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    fn cfg(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs.iter().map(|(k, v)| ((*k).into(), v.clone())).collect()
    }

    #[test]
    fn validate_ok_when_module_exists() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/ExtensionPointA.js"), "export default {}").unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            (
                "targeting",
                json!([{ "target": "EXTENSION::POINT::A", "module": "./src/ExtensionPointA.js" }]),
            ),
        ]);
        validate_configuration(&spec, &configuration, dir.path()).unwrap();
    }

    #[test]
    fn validate_missing_targets() {
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[("name", json!("UI Extension"))]);
        let err = validate_configuration(&spec, &configuration, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("No extension targets"));
    }

    #[test]
    fn validate_duplicate_targets() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.js"), "1").unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            (
                "targeting",
                json!([
                    { "target": "A", "module": "a.js" },
                    { "target": "A", "module": "a.js" }
                ]),
            ),
        ]);
        let err = validate_configuration(&spec, &configuration, dir.path()).unwrap_err();
        assert!(err.to_string().contains("Duplicate extension target"));
    }

    #[test]
    fn validate_missing_module_file() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            (
                "targeting",
                json!([{ "target": "A", "module": "./missing.js" }]),
            ),
        ]);
        let err = validate_configuration(&spec, &configuration, dir.path()).unwrap_err();
        assert!(err.to_string().contains("Couldn't find"));
    }

    #[tokio::test]
    async fn targeting_inherits_metafields_and_build_manifest() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("test-ui-extension")),
            ("api_version", json!("2023-01")),
            (
                "metafields",
                json!([{ "namespace": "test", "key": "test" }]),
            ),
            (
                "targeting",
                json!([{
                    "target": "EXTENSION::POINT::A",
                    "module": "./src/ExtensionPointA.js",
                    "should_render": { "module": "./src/ShouldRender.js" },
                    "default_placement": "PLACEMENT_REFERENCE1"
                }]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        let points = out.get("extension_points").and_then(|v| v.as_array()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(
            points[0]["metafields"],
            json!([{ "namespace": "test", "key": "test" }])
        );
        assert_eq!(
            points[0]["build_manifest"]["assets"]["main"]["filepath"],
            "test-ui-extension.js"
        );
        assert_eq!(
            points[0]["build_manifest"]["assets"]["should_render"]["filepath"],
            "test-ui-extension-conditions.js"
        );
        assert_eq!(
            points[0]["default_placement_reference"],
            "PLACEMENT_REFERENCE1"
        );
    }

    #[test]
    fn should_render_target_helper() {
        assert!(get_should_render_target("purchase.checkout.block.render.should-render"));
        assert!(!get_should_render_target("purchase.checkout.block.render"));
        assert!(get_should_render_target("admin.product-details.action.should-render"));
    }

    #[tokio::test]
    async fn targeting_accepts_urls_preloads_and_capabilities() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("test-ui-extension")),
            ("api_version", json!("2023-01")),
            (
                "targeting",
                json!([{
                    "target": "EXTENSION::POINT::A",
                    "module": "./src/A.js",
                    "urls": { "edit": "/edit" },
                    "preloads": { "chat": "/chat" },
                    "capabilities": { "allow_direct_linking": true },
                    "assets": [{ "name": "icon", "path": "./icon.svg" }]
                }]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        let point = &out["extension_points"][0];
        assert_eq!(point["urls"]["edit"], "/edit");
        assert_eq!(point["preloads"]["chat"], "/chat");
        assert_eq!(point["capabilities"]["allow_direct_linking"], true);
        assert_eq!(point["assets"][0]["name"], "icon");
    }

    #[tokio::test]
    async fn targeting_passes_tools_and_instructions() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("test-ui-extension")),
            (
                "targeting",
                json!([{
                    "target": "EXTENSION::POINT::A",
                    "module": "./src/A.js",
                    "tools": { "module": "./tools.json" },
                    "instructions": { "module": "./instructions.md" }
                }]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        let point = &out["extension_points"][0];
        assert_eq!(point["tools"]["module"], "./tools.json");
        assert_eq!(point["instructions"]["module"], "./instructions.md");
    }

    #[tokio::test]
    async fn deploy_includes_capabilities_and_supported_features() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("test-ui")),
            ("api_version", json!("2023-01")),
            (
                "capabilities",
                json!({
                    "network_access": true,
                    "api_access": false,
                    "block_progress": false
                }),
            ),
            (
                "supported_features",
                json!({ "runs_offline": true }),
            ),
            (
                "targeting",
                json!([{ "target": "purchase.checkout.block.render", "module": "./src/A.js" }]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["capabilities"]["network_access"], true);
        assert_eq!(out["supported_features"]["runs_offline"], true);
        assert_eq!(out["name"], "UI Extension");
        assert_eq!(out["api_version"], "2023-01");
    }

    #[tokio::test]
    async fn supported_features_undefined_when_absent() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("test-ui")),
            (
                "targeting",
                json!([{ "target": "A", "module": "./a.js" }]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert!(out.get("supported_features").unwrap().is_null());
    }

    #[tokio::test]
    async fn multiple_targets_preserve_order() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            ("handle", json!("multi")),
            (
                "targeting",
                json!([
                    { "target": "A", "module": "./a.js" },
                    { "target": "B", "module": "./b.js" }
                ]),
            ),
        ]);
        let out = build_deploy_config(&spec, &configuration, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        let points = out["extension_points"].as_array().unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0]["target"], "A");
        assert_eq!(points[1]["target"], "B");
    }

    #[test]
    fn validate_ok_with_extension_points_alias() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.js"), "1").unwrap();
        let spec = create_extension_specification("ui_extension").unwrap();
        let configuration = cfg(&[
            ("name", json!("UI Extension")),
            (
                "extension_points",
                json!([{ "target": "A", "module": "a.js" }]),
            ),
        ]);
        validate_configuration(&spec, &configuration, dir.path()).unwrap();
    }

    #[test]
    fn spec_has_ui_preview_and_esbuild() {
        let spec = ui_extension_specification();
        assert!(spec
            .features
            .contains(&crate::models::extensions::specification::ExtensionFeature::UiPreview));
        assert!(spec
            .features
            .contains(&crate::models::extensions::specification::ExtensionFeature::Esbuild));
        assert_eq!(spec.registration_limit, 50);
    }
}
