//! Editor extension collection tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::build_deploy_config;
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn collects_includes_handles() {
        let spec = create_extension_specification("editor_extension_collection").unwrap();
        let dir = tempdir().unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("Collection"));
        cfg.insert("handle".into(), json!("col"));
        cfg.insert("includes".into(), json!(["one", "two"]));
        let out = build_deploy_config(&spec, &cfg, dir.path(), &Default::default())
            .await
            .unwrap()
            .unwrap();
        let items = out["in_collection"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["handle"], "one");
    }
}
