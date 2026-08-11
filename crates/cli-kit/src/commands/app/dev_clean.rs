//! `shopify app dev clean` — stop the Next-Gen Dev Session preview.

use app::services::{
    dev_clean, linked_app_context, resolve_primary_store, DevCleanOptions, LinkedAppContextOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;
use crate::constants::app_management_fqdn;
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{AppManagementApiOptions, OAuthApplications, PartnersApiOptions};
use crate::util::fqdn::normalize_store_fqdn;

#[derive(Debug)]
pub struct DevClean {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    store: Option<String>,
}

impl DevClean {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        store: Option<String>,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            store,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for DevClean {
    fn name() -> &'static str {
        "clean"
    }
    fn topic() -> &'static str {
        "app dev"
    }
    fn description() -> &'static str {
        "Cleans up the dev preview from the selected store"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                client_id: self.client_id.clone(),
            },
            client.as_ref(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let store_flag = self.store.as_ref().map(|s| normalize_store_fqdn(s, None));
        let store = resolve_primary_store(&ctx, client.as_ref(), store_flag.as_deref())
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;

        let (app_dev_token, app_dev_graphql_url) = app_dev_credentials().await?;

        dev_clean(
            &ctx,
            client.as_ref(),
            &store,
            DevCleanOptions {
                app_dev_token,
                app_dev_graphql_url,
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))
    }
}

async fn app_dev_credentials() -> Result<(String, String), CliError> {
    let store = SessionStore::new();
    let applications = OAuthApplications {
        app_management_api: Some(AppManagementApiOptions { scopes: vec![] }),
        partners_api: Some(PartnersApiOptions { scopes: vec![] }),
        ..Default::default()
    };
    let tokens = ensure_authenticated(&applications, &store)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
    let token = tokens
        .app_management
        .or(tokens.partners)
        .unwrap_or_default();
    let url = format!(
        "https://{}/app_dev/unstable/graphql.json",
        app_management_fqdn(None)
    );
    Ok((token, url))
}
