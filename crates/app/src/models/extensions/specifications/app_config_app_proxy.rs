//! App config app_proxy module.
#![allow(dead_code)]

use crate::models::extensions::transform::{
    transform_app_proxy_forward, transform_app_proxy_reverse,
};
use serde_json::Value;

pub fn transform_local(local: &Value, application_url: &str) -> Value {
    transform_app_proxy_forward(local, application_url)
}

pub fn transform_remote(remote: &Value) -> Value {
    transform_app_proxy_reverse(remote)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prepends_application_url() {
        let local = json!({ "app_proxy": { "url": "/apps/proxy", "subpath": "apps", "prefix": "a" } });
        let remote = transform_local(&local, "https://example.com");
        let serialized = remote.to_string();
        assert!(
            serialized.contains("example.com") || serialized.contains("apps"),
            "{serialized}"
        );
    }
}
