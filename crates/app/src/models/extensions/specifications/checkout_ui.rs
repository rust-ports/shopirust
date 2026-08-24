//! Checkout UI extension tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::{build_deploy_config, validate_configuration};
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn requires_name() {
        let spec = create_extension_specification("checkout_ui_extension").unwrap();
        let cfg = HashMap::new();
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[tokio::test]
    async fn deploy_includes_extension_points() {
        let spec = create_extension_specification("checkout_ui_extension").unwrap();
        let dir = tempdir().unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("Checkout"));
        cfg.insert("handle".into(), json!("checkout"));
        cfg.insert(
            "extension_points".into(),
            json!(["Checkout::Dynamic::Render"]),
        );
        cfg.insert(
            "metafields".into(),
            json!([{ "namespace": "n", "key": "k" }]),
        );
        let out = build_deploy_config(&spec, &cfg, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["name"], "Checkout");
        assert_eq!(out["extension_points"][0], "Checkout::Dynamic::Render");
        assert_eq!(out["metafields"][0]["namespace"], "n");
    }
}
