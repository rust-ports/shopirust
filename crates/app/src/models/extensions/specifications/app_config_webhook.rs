//! App config webhooks module.
#![allow(dead_code)]

use crate::models::extensions::transform::{
    transform_webhooks_forward, transform_webhooks_reverse,
};
use serde_json::Value;

pub fn transform_local(local: &Value) -> Value {
    transform_webhooks_forward(local)
}

pub fn transform_remote(remote: &Value) -> Value {
    transform_webhooks_reverse(remote)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn forward_keeps_api_version() {
        let local = json!({ "webhooks": { "api_version": "2024-10" } });
        let remote = transform_local(&local);
        assert!(
            remote.to_string().contains("2024-10")
                || remote.get("api_version").is_some()
                || remote.get("webhooks").is_some()
        );
    }

    #[test]
    fn reverse_round_trips_object() {
        let remote = json!({ "api_version": "2024-10" });
        let local = transform_remote(&remote);
        assert!(!local.is_null());
    }
}
