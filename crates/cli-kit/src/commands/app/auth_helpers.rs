//! Shared helpers for app topic commands that need Developer Platform auth.

use app::services::LinkedAppContextOptions;
use cli_api::DeveloperPlatformClient;
use cli_core::error::CliError;
use std::path::PathBuf;

use crate::api::developer_platform::developer_platform_with_business_platform;
use crate::api::webhooks::WebhooksClient;
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{
    AppManagementApiOptions, BusinessPlatformApiOptions, OAuthApplications, PartnersApiOptions,
};

pub async fn authenticated_developer_platform() -> Result<Box<dyn DeveloperPlatformClient>, CliError>
{
    let store = SessionStore::new();
    let applications = OAuthApplications {
        app_management_api: Some(AppManagementApiOptions { scopes: vec![] }),
        partners_api: Some(PartnersApiOptions { scopes: vec![] }),
        business_platform_api: Some(BusinessPlatformApiOptions::default()),
        ..Default::default()
    };
    let tokens = ensure_authenticated(&applications, &store)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

    let am_token = tokens
        .app_management
        .clone()
        .or(tokens.partners.clone())
        .unwrap_or_default();
    Ok(developer_platform_with_business_platform(
        tokens.partners,
        am_token,
        tokens.business_platform,
        Default::default(),
    ))
}

/// Authenticated webhooks GraphQL client for sample / topic / version requests.
pub async fn authenticated_webhooks_client(
    organization_id: &str,
) -> Result<WebhooksClient, CliError> {
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
    Ok(WebhooksClient::new(
        organization_id.to_string(),
        token,
        None,
    ))
}

pub fn linked_ctx_options(
    path: &str,
    config: Option<String>,
    client_id: Option<String>,
    reset: bool,
) -> LinkedAppContextOptions {
    LinkedAppContextOptions {
        directory: PathBuf::from(path),
        config_name: config,
        client_id,
        force_relink: reset,
    }
}

pub fn admin_graphql_url(store: &str, version: &str) -> String {
    let fqdn = if store.contains('.') {
        store.to_string()
    } else {
        format!("{store}.myshopify.com")
    };
    format!("https://{fqdn}/admin/api/{version}/graphql.json")
}
