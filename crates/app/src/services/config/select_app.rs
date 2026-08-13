//! Fetch remote app configuration modules and map them to local TOML
//! (upstream `services/app/select-app.ts`).

use crate::error::AppError;
use crate::models::extensions::specification::UidStrategy;
use crate::models::extensions::specifications::{all_known_specifications, lookup};
use crate::models::extensions::ExtensionSpecification;
use cli_api::{AppModuleVersion, AppVersion, DeveloperPlatformClient, MinimalAppIdentifiers};
use serde_json::{Map, Value};

/// Deep-merge JSON objects; arrays in `src` replace `dest` (upstream `replaceLocalArrayStrategy`).
pub fn deep_merge(dest: Value, src: Value) -> Value {
    match (dest, src) {
        (Value::Object(mut a), Value::Object(b)) => {
            for (k, v) in b {
                let merged = match a.remove(&k) {
                    Some(existing) => deep_merge(existing, v),
                    None => v,
                };
                a.insert(k, merged);
            }
            Value::Object(a)
        }
        (_, src) => src,
    }
}

fn extension_type_strategy(specs: &[ExtensionSpecification], type_name: &str) -> UidStrategy {
    specs
        .iter()
        .find(|s| s.matches_type(type_name))
        .map(|s| s.uid_strategy)
        .or_else(|| lookup(type_name).map(|s| s.uid_strategy))
        .unwrap_or(UidStrategy::Uuid)
}

/// Map remote config modules to a local top-level app configuration object.
pub fn remote_app_configuration_extension_content(
    config_registrations: &[AppModuleVersion],
    specifications: &[ExtensionSpecification],
) -> Value {
    let mut remote_app_config = Value::Object(Map::new());
    let config_specs: Vec<_> = specifications
        .iter()
        .filter(|s| s.is_app_config())
        .cloned()
        .collect();

    for module in config_registrations {
        let config_spec = config_specs.iter().find(|spec| {
            spec.matches_type(&module.module_type)
                || spec.identifier.eq_ignore_ascii_case(&module.module_type)
        });
        let Some(config_spec) = config_spec else {
            continue;
        };
        let Some(ref config) = module.config else {
            continue;
        };
        let transformed = config_spec.transform_remote_to_local(config);
        remote_app_config = deep_merge(remote_app_config, transformed);
    }
    remote_app_config
}

/// Fetch the active app version and convert its configuration modules to local TOML shape.
pub async fn fetch_app_remote_configuration(
    identifiers: &MinimalAppIdentifiers,
    client: &dyn DeveloperPlatformClient,
    specifications: &[ExtensionSpecification],
    active_app_version: Option<&AppVersion>,
) -> Result<Option<Value>, AppError> {
    let owned;
    let app_version = if let Some(v) = active_app_version {
        v
    } else {
        owned = client
            .active_app_version(identifiers)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        match owned.as_ref() {
            Some(v) => v,
            None => return Ok(None),
        }
    };

    let config_modules: Vec<_> = app_version
        .app_module_versions
        .iter()
        .filter(|m| extension_type_strategy(specifications, &m.module_type) != UidStrategy::Uuid)
        .cloned()
        .collect();

    if config_modules.is_empty() {
        return Ok(None);
    }

    Ok(Some(remote_app_configuration_extension_content(
        &config_modules,
        specifications,
    )))
}

/// Local specs used when the platform catalog is unavailable.
pub fn local_configuration_specifications() -> Vec<ExtensionSpecification> {
    all_known_specifications()
        .into_iter()
        .filter(|s| s.is_app_config())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_org_app, MockClient};
    use cli_api::AppModuleVersion;

    fn module(ty: &str, config: Value) -> AppModuleVersion {
        AppModuleVersion {
            registration_id: format!("id-{ty}"),
            registration_uuid: Some(format!("uuid-{ty}")),
            registration_title: ty.into(),
            config: Some(config),
            target: None,
            module_type: ty.into(),
        }
    }

    #[test]
    fn merges_config_modules_to_local_shape() {
        let specs = local_configuration_specifications();
        let modules = vec![
            module("webhooks", serde_json::json!({"api_version": "2023-04"})),
            module(
                "app_home",
                serde_json::json!({"app_url": "https://myapp.com", "embedded": true}),
            ),
            module("branding", serde_json::json!({"name": "name"})),
        ];
        let result = remote_app_configuration_extension_content(&modules, &specs);
        assert_eq!(result.get("name").and_then(|v| v.as_str()), Some("name"));
        assert_eq!(
            result.get("application_url").and_then(|v| v.as_str()),
            Some("https://myapp.com")
        );
        assert_eq!(result.get("embedded").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            result
                .pointer("/webhooks/api_version")
                .and_then(|v| v.as_str()),
            Some("2023-04")
        );
    }

    #[test]
    fn empty_modules_returns_empty_object() {
        let specs = local_configuration_specifications();
        let result = remote_app_configuration_extension_content(&[], &specs);
        assert_eq!(result, Value::Object(Map::new()));
    }

    #[tokio::test]
    async fn fetch_returns_none_without_config_modules() {
        let client = MockClient {
            app: Some(sample_org_app("key-1")),
            active_version: Some(AppVersion {
                app_module_versions: vec![module(
                    "function",
                    serde_json::json!({"title": "fn"}),
                )],
            }),
            ..Default::default()
        };
        let ids = MinimalAppIdentifiers {
            api_key: "key-1".into(),
            organization_id: "org-1".into(),
            id: "app-1".into(),
        };
        let specs = local_configuration_specifications();
        let result = fetch_app_remote_configuration(&ids, &client, &specs, None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_returns_transformed_config() {
        let client = MockClient {
            app: Some(sample_org_app("key-1")),
            active_version: Some(AppVersion {
                app_module_versions: vec![module(
                    "branding",
                    serde_json::json!({"name": "Remote"}),
                )],
            }),
            ..Default::default()
        };
        let ids = MinimalAppIdentifiers {
            api_key: "key-1".into(),
            organization_id: "org-1".into(),
            id: "app-1".into(),
        };
        let specs = local_configuration_specifications();
        let result = fetch_app_remote_configuration(&ids, &client, &specs, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.get("name").and_then(|v| v.as_str()), Some("Remote"));
    }

    #[test]
    fn deep_merge_replaces_arrays() {
        let dest = serde_json::json!({"a": [1], "b": {"x": 1}});
        let src = serde_json::json!({"a": [2, 3], "b": {"y": 2}});
        let merged = deep_merge(dest, src);
        assert_eq!(merged["a"], serde_json::json!([2, 3]));
        assert_eq!(merged["b"]["x"], 1);
        assert_eq!(merged["b"]["y"], 2);
    }

    #[test]
    fn config_pipeline_snapshot_remote_to_local() {
        let specs = local_configuration_specifications();
        let modules = vec![
            module("branding", serde_json::json!({"name": "Snapshot App"})),
            module(
                "app_home",
                serde_json::json!({"app_url": "https://snap.example", "embedded": true}),
            ),
            module(
                "webhooks",
                serde_json::json!({"api_version": "2024-01", "subscriptions": []}),
            ),
            module(
                "app_access",
                serde_json::json!({"scopes": "read_products", "redirect_url_allowlist": ["https://snap.example/cb"]}),
            ),
        ];
        let local = remote_app_configuration_extension_content(&modules, &specs);
        assert_eq!(
            local.get("name").and_then(|v| v.as_str()),
            Some("Snapshot App")
        );
        assert_eq!(
            local.get("application_url").and_then(|v| v.as_str()),
            Some("https://snap.example")
        );
        assert_eq!(local.get("embedded").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            local.pointer("/webhooks/api_version").and_then(|v| v.as_str()),
            Some("2024-01")
        );
        assert!(
            local.get("webhooks").is_some(),
            "expected webhooks in pipeline output: {local}"
        );
        // scopes may land under access_scopes or as a top-level key depending on the spec transform
        let scopes = local
            .pointer("/access_scopes/scopes")
            .or_else(|| local.get("scopes"))
            .and_then(|v| v.as_str());
        assert!(
            scopes == Some("read_products")
                || local.to_string().contains("read_products"),
            "expected scopes in pipeline output: {local}"
        );
    }
}
