use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification, UidStrategy,
};

/// Function API type aliases (upstream `additionalIdentifiers`).
pub const FUNCTION_ALIASES: &[&str] = &[
    "order_discounts",
    "cart_checkout_validation",
    "cart_transform",
    "delivery_customization",
    "payment_customization",
    "product_discounts",
    "shipping_discounts",
    "fulfillment_constraints",
    "order_routing_location_rule",
    "local_pickup_delivery_option_generator",
    "pickup_point_delivery_option_generator",
];

pub fn function_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "function".into(),
        external_identifier: "function_external".into(),
        external_name: "Function".into(),
        partners_web_identifier: "function".into(),
        surface: "admin".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: FUNCTION_ALIASES.iter().map(|s| (*s).to_string()).collect(),
        group: Some("Functions".into()),
        features: vec![ExtensionFeature::Function],
        uid_strategy: UidStrategy::Uuid,
        graph_ql_type: None,
        dependency: None,
        json_schema: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::deploy::{
        build_deploy_config, validate_configuration, DeployConfigContext,
    };
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn function_cfg() -> HashMap<String, serde_json::Value> {
        let mut c = HashMap::new();
        c.insert("name".into(), json!("Discount"));
        c.insert("type".into(), json!("function"));
        c.insert("api_version".into(), json!("2024-10"));
        c.insert("handle".into(), json!("discount"));
        c
    }

    #[test]
    fn aliases_match_function_spec() {
        let spec = function_specification();
        for alias in FUNCTION_ALIASES {
            assert!(spec.matches_type(alias), "{alias}");
        }
    }

    #[test]
    fn requires_api_version() {
        let spec = create_extension_specification("function").unwrap();
        let mut cfg = function_cfg();
        cfg.remove("api_version");
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("api_version"));
    }

    #[tokio::test]
    async fn deploy_reads_input_query_and_ui() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("input.graphql"), "query { cart { id } }").unwrap();
        let spec = create_extension_specification("function").unwrap();
        let mut cfg = function_cfg();
        cfg.insert("description".into(), json!("A discount"));
        cfg.insert(
            "ui".into(),
            json!({ "enable_create": false, "paths": { "create": "/create" } }),
        );
        let ctx = DeployConfigContext {
            api_key: "key".into(),
            module_id: Some("mid".into()),
            ..Default::default()
        };
        let out = build_deploy_config(&spec, &cfg, dir.path(), &ctx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["title"], "Discount");
        assert_eq!(out["module_id"], "mid");
        assert_eq!(out["app_key"], "key");
        assert!(out["input_query"].as_str().unwrap().contains("cart"));
        assert_eq!(out["enable_creation_ui"], false);
    }

    #[tokio::test]
    async fn deploy_maps_targets() {
        let dir = tempdir().unwrap();
        let spec = create_extension_specification("function").unwrap();
        let mut cfg = function_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": "cart.lines.discounts.generate.run" }]),
        );
        let out = build_deploy_config(&spec, &cfg, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            out["targets"][0]["handle"],
            "cart.lines.discounts.generate.run"
        );
    }
}
