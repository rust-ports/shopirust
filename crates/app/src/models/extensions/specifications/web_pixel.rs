//! Web pixel extension tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::{build_deploy_config, validate_configuration};
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    fn cfg() -> HashMap<String, serde_json::Value> {
        let mut c = HashMap::new();
        c.insert("name".into(), json!("Pixel"));
        c.insert("handle".into(), json!("pixel"));
        c.insert("runtime_context".into(), json!("strict"));
        c.insert("settings".into(), json!({ "type": "object" }));
        c
    }

    #[test]
    fn requires_runtime_context() {
        let spec = create_extension_specification("web_pixel_extension").unwrap();
        let mut c = cfg();
        c.remove("runtime_context");
        let err = validate_configuration(&spec, &c, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("runtime_context"));
    }

    #[test]
    fn rejects_deprecated_configuration_key() {
        let spec = create_extension_specification("web_pixel_extension").unwrap();
        let mut c = cfg();
        c.insert("configuration".into(), json!({}));
        let err = validate_configuration(&spec, &c, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("deprecated"));
    }

    #[tokio::test]
    async fn deploy_maps_settings() {
        let spec = create_extension_specification("web_pixel_extension").unwrap();
        let out = build_deploy_config(&spec, &cfg(), Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["runtime_context"], "strict");
        assert_eq!(out["runtime_configuration_definition"]["type"], "object");
    }
}
