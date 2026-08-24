//! Tax calculation extension tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::{build_deploy_config, validate_configuration};
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    fn cfg() -> HashMap<String, serde_json::Value> {
        let mut c = HashMap::new();
        c.insert("name".into(), json!("Tax"));
        c.insert("handle".into(), json!("tax"));
        c.insert(
            "production_api_base_url".into(),
            json!("https://tax.example"),
        );
        c.insert("calculate_taxes_api_endpoint".into(), json!("/calculate"));
        c.insert("api_version".into(), json!("2024-10"));
        c
    }

    #[test]
    fn requires_production_url() {
        let spec = create_extension_specification("tax_calculation").unwrap();
        let mut c = cfg();
        c.remove("production_api_base_url");
        let err = validate_configuration(&spec, &c, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("production_api_base_url"));
    }

    #[tokio::test]
    async fn deploy_includes_endpoints() {
        let spec = create_extension_specification("tax_calculation").unwrap();
        let out = build_deploy_config(&spec, &cfg(), Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["production_api_base_url"], "https://tax.example");
        assert_eq!(out["calculate_taxes_api_endpoint"], "/calculate");
    }
}
