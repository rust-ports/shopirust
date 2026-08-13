//! Point of sale app config tests.

#[cfg(test)]
mod tests {
    use crate::models::extensions::deploy::build_deploy_config;
    use crate::models::extensions::specification::create_extension_specification;
    use crate::models::extensions::transform::{app_config_transform, point_of_sale_transform};
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn transform_embedded_flag() {
        let spec = create_extension_specification("point_of_sale").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("pos".into(), json!({ "embedded": true }));
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["embedded"], true);
        let local = json!({ "pos": { "embedded": true } });
        let remote = app_config_transform(&local, &point_of_sale_transform(), false);
        assert_eq!(remote["embedded"], true);
        let back = app_config_transform(&remote, &point_of_sale_transform(), true);
        assert_eq!(back["pos"]["embedded"], true);
    }
}
