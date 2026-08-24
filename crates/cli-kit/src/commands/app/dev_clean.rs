//! `shopify app dev clean` — stop the Next-Gen Dev Session preview.

use app::services::{
    dev_clean, linked_app_context, store_context, DevCleanOptions, StoreContextOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;
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
    reset: bool,
}

impl DevClean {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        store: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            store,
            reset,
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
        let prompter = CliKitPrompter;
        let ctx = linked_app_context(
            linked_ctx_options(
                &self.path,
                self.config.clone(),
                self.client_id.clone(),
                self.reset,
            ),
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let store_flag = self.store.as_ref().map(|s| normalize_store_fqdn(s, None));
        let store = store_context(
            &ctx,
            client.as_ref(),
            StoreContextOptions {
                store_fqdn: store_flag,
                force_reselect_store: self.reset,
                ..Default::default()
            },
            Some(&prompter),
        )
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
