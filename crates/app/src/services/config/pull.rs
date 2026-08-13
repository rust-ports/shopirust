use crate::error::AppError;
use crate::models::loader::{load_app, LoadAppOptions};
use crate::models::AppConfiguration;
use crate::services::config::select_app::{
    deep_merge, fetch_app_remote_configuration, local_configuration_specifications,
};
use crate::services::config::write_app_configuration_file;
use cli_api::{DeveloperPlatformClient, MinimalAppIdentifiers, OrganizationApp};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PullConfigOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    /// When set, skip the platform fetch and merge this JSON instead (tests).
    pub remote_configuration: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct PullConfigResult {
    pub config_path: PathBuf,
    pub updated: bool,
}

/// Pull remote app configuration into the local TOML file using the same
/// remote-module merge as `config link`.
pub async fn pull_config(
    options: PullConfigOptions,
    client: Option<&dyn DeveloperPlatformClient>,
    remote_app: Option<&OrganizationApp>,
) -> Result<PullConfigResult, AppError> {
    let loaded = load_app(LoadAppOptions {
        directory: options.directory.clone(),
        config_name: options.config_name.clone(),
        ignore_unknown_extensions: false,
    })?;

    let remote_json = if let Some(remote) = options.remote_configuration {
        remote
    } else {
        let (client, remote_app) = match (client, remote_app) {
            (Some(c), Some(a)) => (c, a),
            _ => {
                return Ok(PullConfigResult {
                    config_path: loaded.configuration_path,
                    updated: false,
                });
            }
        };
        let identifiers = MinimalAppIdentifiers {
            api_key: remote_app.api_key.clone(),
            organization_id: remote_app
                .organization_id
                .clone()
                .unwrap_or_default(),
            id: remote_app.id.clone(),
        };
        let specs = local_configuration_specifications();
        fetch_app_remote_configuration(&identifiers, client, &specs, None)
            .await?
            .unwrap_or_else(|| {
                serde_json::json!({
                    "name": remote_app.title,
                    "application_url": remote_app.application_url,
                })
            })
    };

    let local = serde_json::to_value(&loaded.configuration).unwrap_or(Value::Null);
    let merged = deep_merge(local, remote_json);
    let cfg: AppConfiguration = serde_json::from_value(merged)
        .map_err(|e| AppError::message(format!("merge remote config: {e}")))?;

    let previous = serde_json::to_value(&loaded.configuration).ok();
    let next = serde_json::to_value(&cfg).ok();
    let updated = previous != next;
    if updated {
        write_app_configuration_file(&loaded.configuration_path, &cfg)?;
    }

    Ok(PullConfigResult {
        config_path: loaded.configuration_path,
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_org_app, MockClient};
    use cli_api::{AppModuleVersion, AppVersion};
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn pull_merges_remote_fields() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Old\"\napplication_url = \"https://old.example\"\n",
        )
        .unwrap();
        let result = pull_config(
            PullConfigOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                remote_configuration: Some(serde_json::json!({
                    "name": "New",
                    "application_url": "https://new.example",
                    "access_scopes": { "scopes": "write_products" },
                })),
            },
            None,
            None,
        )
        .await
        .unwrap();
        assert!(result.updated);
        let content = fs::read_to_string(result.config_path).unwrap();
        assert!(content.contains("name = \"New\""));
        assert!(content.contains("https://new.example"));
    }

    #[tokio::test]
    async fn pull_fetches_remote_modules() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Old\"\n",
        )
        .unwrap();
        let mut client = MockClient::with_app(sample_org_app("abc"));
        client.active_version = Some(AppVersion {
            app_module_versions: vec![AppModuleVersion {
                registration_id: "1".into(),
                registration_uuid: Some("u".into()),
                registration_title: "branding".into(),
                config: Some(serde_json::json!({"name": "From Remote"})),
                target: None,
                module_type: "branding".into(),
            }],
        });
        let app = sample_org_app("abc");
        let result = pull_config(
            PullConfigOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                remote_configuration: None,
            },
            Some(&client),
            Some(&app),
        )
        .await
        .unwrap();
        assert!(result.updated);
        let content = fs::read_to_string(result.config_path).unwrap();
        assert!(content.contains("From Remote"));
    }
}
