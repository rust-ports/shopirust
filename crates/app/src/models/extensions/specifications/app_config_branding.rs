//! App config branding module.
#![allow(dead_code)]

use crate::models::extensions::specification::ExtensionSpecification;
use crate::models::extensions::transform::{app_config_transform, branding_transform};
use serde_json::Value;

pub fn branding_specification() -> ExtensionSpecification {
    super::lookup("branding").expect("branding spec")
}

pub fn transform_local(local: &Value) -> Value {
    app_config_transform(local, &branding_transform(), false)
}

pub fn transform_remote(remote: &Value) -> Value {
    app_config_transform(remote, &branding_transform(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn local_to_remote_maps_handle() {
        let local = json!({ "name": "My App", "handle": "my-app" });
        let remote = transform_local(&local);
        assert_eq!(
            remote.get("app_handle").and_then(|v| v.as_str()),
            Some("my-app")
        );
    }

    #[test]
    fn branding_name_max_length_is_validated_elsewhere() {
        let spec = branding_specification();
        assert_eq!(spec.identifier, "branding");
    }
}
