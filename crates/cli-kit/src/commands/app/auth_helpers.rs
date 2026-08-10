//! Shared helpers for app topic commands that need Developer Platform auth.

use cli_api::{
    select_developer_platform_client, DeveloperPlatformClient, SelectDeveloperPlatformClientOptions,
};
use cli_core::error::CliError;

use crate::api::app_management::AppManagementClient;
use crate::api::developer_platform::{AppManagementPlatformClient, PartnersPlatformClient};
use crate::api::partners::PartnersClient;
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{AppManagementApiOptions, OAuthApplications, PartnersApiOptions};

pub async fn authenticated_developer_platform() -> Result<Box<dyn DeveloperPlatformClient>, CliError>
{
    let store = SessionStore::new();
    let applications = OAuthApplications {
        app_management_api: Some(AppManagementApiOptions { scopes: vec![] }),
        partners_api: Some(PartnersApiOptions { scopes: vec![] }),
        ..Default::default()
    };
    let tokens = ensure_authenticated(&applications, &store)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

    let partners = tokens.partners.clone().map(|tok| {
        Box::new(PartnersPlatformClient::new(PartnersClient::new_with_token(
            tok, None,
        ))) as Box<dyn DeveloperPlatformClient>
    });
    let am_token = tokens
        .app_management
        .or(tokens.partners)
        .unwrap_or_default();
    let app_management = Box::new(AppManagementPlatformClient::new(AppManagementClient::new(
        am_token, None,
    ))) as Box<dyn DeveloperPlatformClient>;

    Ok(select_developer_platform_client(
        SelectDeveloperPlatformClientOptions::default(),
        partners,
        app_management,
    ))
}

pub fn admin_graphql_url(store: &str, version: &str) -> String {
    let fqdn = if store.contains('.') {
        store.to_string()
    } else {
        format!("{store}.myshopify.com")
    };
    format!("https://{fqdn}/admin/api/{version}/graphql.json")
}
