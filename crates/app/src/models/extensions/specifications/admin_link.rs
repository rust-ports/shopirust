//! Admin link contract module tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::build_deploy_config;
    use crate::models::extensions::specification::{
        create_extension_specification, ExtensionFeature,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn strips_first_class_fields() {
        let mut spec = create_extension_specification("admin_link").unwrap();
        spec.features = vec![ExtensionFeature::Localization];
        let mut cfg = HashMap::new();
        cfg.insert("type".into(), json!("admin_link"));
        cfg.insert("handle".into(), json!("link"));
        cfg.insert("name".into(), json!("Link"));
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert!(out.get("type").is_none());
        assert_eq!(out.get("name").and_then(|v| v.as_str()), Some("Link"));
    }
}
