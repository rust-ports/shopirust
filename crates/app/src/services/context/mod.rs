pub mod breakdown_extensions;
pub mod id_matching;
pub mod id_manual_matching;
pub mod identifiers;

pub use id_matching::{
    automatic_matchmaking, LocalSource, MatchResult, RemoteSource as MatchRemoteSource,
};

use crate::error::AppError;
use crate::local_storage::{set_cached_app_info, CachedAppInfo};
use crate::models::loader::{load_app, LoadAppOptions, LoadedApp};
use crate::prompts::Prompter;
use crate::services::config::{link_config, LinkConfigOptions};
use cli_api::{DeveloperPlatformClient, Organization, OrganizationApp};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LinkedAppContextOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    pub client_id: Option<String>,
    pub force_relink: bool,
}

#[derive(Debug, Clone)]
pub struct LinkedAppContext {
    pub app: LoadedApp,
    pub remote_app: OrganizationApp,
    pub organization: Organization,
}

/// Load the local app and resolve the linked remote app via the platform client.
///
/// `--reset` (`force_relink`) re-runs config link. An unlinked local config also forces a link.
pub async fn linked_app_context(
    options: LinkedAppContextOptions,
    client: &dyn DeveloperPlatformClient,
    prompter: Option<&dyn Prompter>,
) -> Result<LinkedAppContext, AppError> {
    let mut config_name = options.config_name.clone();

    if options.force_relink {
        let linked = link_config(
            LinkConfigOptions {
                directory: options.directory.clone(),
                client_id: options.client_id.clone(),
                config_name: config_name.clone(),
                app_name: None,
                organization_id: None,
                is_new_app: false,
            },
            client,
            prompter,
        )
        .await?;
        config_name = Some(linked.config_file);
    }

    let app = match load_app(LoadAppOptions {
        directory: options.directory.clone(),
        config_name: config_name.clone(),
        ignore_unknown_extensions: false,
    }) {
        Ok(app) => app,
        Err(_) if !options.force_relink => {
            let linked = link_config(
                LinkConfigOptions {
                    directory: options.directory.clone(),
                    client_id: options.client_id.clone(),
                    config_name: config_name.clone(),
                    app_name: None,
                    organization_id: None,
                    is_new_app: false,
                },
                client,
                prompter,
            )
            .await?;
            load_app(LoadAppOptions {
                directory: options.directory.clone(),
                config_name: Some(linked.config_file),
                ignore_unknown_extensions: false,
            })?
        }
        Err(e) => return Err(e),
    };

    if !options.force_relink && !app.is_linked() {
        let linked = link_config(
            LinkConfigOptions {
                directory: options.directory.clone(),
                client_id: options.client_id.clone(),
                config_name: config_name.clone(),
                app_name: None,
                organization_id: None,
                is_new_app: false,
            },
            client,
            prompter,
        )
        .await?;
        return Box::pin(linked_app_context(
            LinkedAppContextOptions {
                directory: options.directory,
                config_name: Some(linked.config_file),
                client_id: options.client_id,
                force_relink: false,
            },
            client,
            prompter,
        ))
        .await;
    }

    let api_key = options
        .client_id
        .clone()
        .or_else(|| app.configuration.client_id.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::message(
                "No client_id found. Run `shopify app config link` or pass --client-id.",
            )
        })?;

    let remote_app = client
        .app_from_identifiers(&api_key)
        .await
        .map_err(|e| AppError::message(e.to_string()))?
        .ok_or_else(|| AppError::message(format!("Invalid Client ID: {api_key}")))?;

    let org_id = remote_app.organization_id.clone().unwrap_or_default();

    let organization = if org_id.is_empty() {
        Organization {
            id: String::new(),
            business_name: "Unknown".into(),
            source: client.organization_source(),
        }
    } else {
        client
            .org_from_id(&org_id)
            .await
            .map_err(|e| AppError::message(e.to_string()))?
            .unwrap_or(Organization {
                id: org_id,
                business_name: "Unknown".into(),
                source: client.organization_source(),
            })
    };

    let cached = crate::local_storage::get_cached_app_info(&options.directory);
    let right_app = remote_app.api_key == app.configuration.client_id.clone().unwrap_or_default();
    if cached.is_none() || right_app {
        let _ = set_cached_app_info(&CachedAppInfo {
            directory: options.directory.display().to_string(),
            config_file: None,
            app_id: Some(remote_app.api_key.clone()),
            title: Some(remote_app.title.clone()),
            org_id: remote_app.organization_id.clone(),
            store_fqdn: None,
            ..Default::default()
        });
    }

    Ok(LinkedAppContext {
        app,
        remote_app,
        organization,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_org_app, MockClient};
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn linked_context_resolves_remote_app() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let client = MockClient::with_app(sample_org_app("key-1"));
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        assert_eq!(ctx.remote_app.api_key, "key-1");
        assert_eq!(ctx.organization.business_name, "Acme");
    }

    #[tokio::test]
    async fn force_relink_rewrites_config() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Old\"\napplication_url = \"https://old.example\"\n",
        )
        .unwrap();
        let client = MockClient::with_app(sample_org_app("key-1"));
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: Some("key-1".into()),
                force_relink: true,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        assert_eq!(ctx.remote_app.api_key, "key-1");
        let raw = fs::read_to_string(dir.path().join("shopify.app.toml")).unwrap();
        assert!(raw.contains("client_id = \"key-1\""));
    }

    #[tokio::test]
    async fn unlinked_config_triggers_link() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "name = \"Unlinked\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let client = MockClient::with_app(sample_org_app("key-1"));
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: Some("key-1".into()),
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        assert_eq!(ctx.remote_app.api_key, "key-1");
        assert!(ctx.app.is_linked());
    }

    #[tokio::test]
    async fn client_id_flag_overrides_toml() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let mut other = sample_org_app("key-2");
        other.id = "app-2".into();
        other.title = "Other".into();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.app = Some(other.clone());
        // app_from_identifiers filters by api_key on MockClient.app — put key-2 there
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: Some("key-2".into()),
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        assert_eq!(ctx.remote_app.api_key, "key-2");
    }
}
