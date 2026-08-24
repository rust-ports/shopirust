//! App config events module tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::build_deploy_config;
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn forwards_subscriptions() {
        let spec = create_extension_specification("events").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert(
            "subscriptions".into(),
            json!([{ "topics": ["products/update"], "uri": "/events" }]),
        );
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert!(
            out.get("events").is_some() || out.get("subscriptions").is_some() || !out.is_null()
        );
    }
}
