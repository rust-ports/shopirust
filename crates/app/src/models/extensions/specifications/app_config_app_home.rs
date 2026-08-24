//! App config app_home module.
#![allow(dead_code)]

use crate::models::extensions::transform::{app_config_transform, app_home_transform};
use serde_json::Value;

pub fn transform_local(local: &Value) -> Value {
    app_config_transform(local, &app_home_transform(), false)
}

pub fn transform_remote(remote: &Value) -> Value {
    app_config_transform(remote, &app_home_transform(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::deploy::validate_configuration;
    use crate::models::extensions::specification::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn requires_application_url_and_embedded() {
        let spec = create_extension_specification("app_home").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("application_url".into(), json!("https://example.com"));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("embedded"));
    }

    #[test]
    fn transform_is_stable() {
        let local = json!({ "application_url": "https://example.com", "embedded": true });
        let remote = transform_local(&local);
        assert_eq!(transform_remote(&remote)["embedded"], true);
    }
}
