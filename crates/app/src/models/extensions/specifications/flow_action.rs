//! Flow action specification tests (upstream `flow_action.test.ts`).

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::{
        build_deploy_config, validate_configuration, DeployConfigContext,
    };
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn requires_name_and_runtime_url() {
        let spec = create_extension_specification("flow_action").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("Action"));
        cfg.insert("handle".into(), json!("action"));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("runtime_url"));
    }

    #[tokio::test]
    async fn deploy_prepends_application_url() {
        let spec = create_extension_specification("flow_action").unwrap();
        let dir = tempdir().unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("Action"));
        cfg.insert("handle".into(), json!("action"));
        cfg.insert("runtime_url".into(), json!("/flow/run"));
        let ctx = DeployConfigContext {
            app_configuration: Some(json!({ "application_url": "https://app.example" })),
            ..Default::default()
        };
        let out = build_deploy_config(&spec, &cfg, dir.path(), &ctx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["url"], "https://app.example/flow/run");
        assert_eq!(out["title"], "Action");
    }
}
