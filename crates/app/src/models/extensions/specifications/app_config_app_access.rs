//! App config app_access module.
#![allow(dead_code)]

use crate::models::extensions::transform::{app_access_transform, app_config_transform};
use serde_json::Value;

pub fn transform_local(local: &Value) -> Value {
    app_config_transform(local, &app_access_transform(), false)
}

pub fn transform_remote(remote: &Value) -> Value {
    app_config_transform(remote, &app_access_transform(), true)
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
    fn requires_redirect_urls() {
        let spec = create_extension_specification("app_access").unwrap();
        let cfg = HashMap::new();
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("redirect_urls"));
    }

    #[test]
    fn transform_round_trip() {
        let local = json!({ "auth": { "redirect_urls": ["https://example.com/cb"] } });
        let remote = transform_local(&local);
        assert!(!remote.is_null());
    }
}
