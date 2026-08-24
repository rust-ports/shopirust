//! Privacy compliance webhooks config tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::build_deploy_config;
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn forwards_privacy_urls() {
        let spec = create_extension_specification("privacy_compliance_webhooks").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert(
            "webhooks".into(),
            json!({
                "privacy_compliance": {
                    "customer_deletion_url": "https://example.com/redact",
                    "shop_deletion_url": "https://example.com/shop-redact"
                }
            }),
        );
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["customers_redact_url"], "https://example.com/redact");
        assert_eq!(out["shop_redact_url"], "https://example.com/shop-redact");
    }
}
