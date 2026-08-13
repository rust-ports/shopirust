//! `shopify app dev` — run the app with hot reload / tunnel / extension preview.

use app::services::{
    dev, get_available_tcp_port, get_tunnel_mode, linked_app_context, store_context,
    DevOptions, StoreContextOptions, TunnelMode, TunnelModeFlags,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::{
    authenticated_developer_platform, authenticated_webhooks_client, linked_ctx_options,
};
use super::webhook::webhooks_sample_client;
use super::prompter::CliKitPrompter;
use crate::constants::app_management_fqdn;
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{AppManagementApiOptions, OAuthApplications, PartnersApiOptions};
use crate::util::fqdn::normalize_store_fqdn;
use crate::util::tunnel::{CloudflareTunnel, TunnelClient};

#[derive(Debug)]
pub struct Dev {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    store: Option<String>,
    tunnel_url: Option<String>,
    use_localhost: bool,
    localhost_port: Option<u16>,
    skip_dependencies_installation: bool,
    theme: Option<String>,
    theme_extension_port: Option<u16>,
    no_update: bool,
    checkout_cart_url: Option<String>,
    subscription_product_url: Option<String>,
    notify: Option<String>,
    graphiql_port: Option<u16>,
    graphiql_key: Option<String>,
    reset: bool,
}

impl Dev {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        store: Option<String>,
        tunnel_url: Option<String>,
        use_localhost: bool,
        localhost_port: Option<u16>,
        skip_dependencies_installation: bool,
        theme: Option<String>,
        theme_extension_port: Option<u16>,
        no_update: bool,
        checkout_cart_url: Option<String>,
        subscription_product_url: Option<String>,
        notify: Option<String>,
        graphiql_port: Option<u16>,
        graphiql_key: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            store,
            tunnel_url,
            use_localhost,
            localhost_port,
            skip_dependencies_installation,
            theme,
            theme_extension_port,
            no_update,
            checkout_cart_url,
            subscription_product_url,
            notify,
            graphiql_port,
            graphiql_key,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Dev {
    fn name() -> &'static str {
        "dev"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Run the app"
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

        let tunnel = get_tunnel_mode(TunnelModeFlags {
            tunnel_url: self.tunnel_url.clone(),
            use_localhost: self.use_localhost,
            localhost_port: self.localhost_port,
        })
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let (tunnel_url_override, tunnel_local_port, mut owned_tunnel) =
            start_tunnel_if_needed(&tunnel).await?;

        let (app_dev_token, app_dev_graphql_url) = app_dev_credentials().await?;
        let webhook_sample_client = authenticated_webhooks_client(&ctx.organization.id)
            .await
            .ok()
            .map(webhooks_sample_client);

        let result = dev(
            &ctx,
            client.as_ref(),
            &store,
            DevOptions {
                directory: PathBuf::from(&self.path),
                update: !self.no_update,
                skip_dependencies_installation: self.skip_dependencies_installation,
                subscription_product_url: self.subscription_product_url.clone(),
                checkout_cart_url: self.checkout_cart_url.clone(),
                tunnel: tunnel.clone(),
                tunnel_url_override,
                tunnel_local_port,
                theme: self.theme.clone(),
                theme_extension_port: self.theme_extension_port,
                notify: self.notify.clone(),
                graphiql_port: self.graphiql_port,
                graphiql_key: self.graphiql_key.clone(),
                app_dev_token,
                app_dev_graphql_url,
                webhook_sample_client,
            },
        )
        .await;

        if let Some(ref mut t) = owned_tunnel {
            t.stop().await;
        }

        result.map_err(|e| CliError::abort(e.to_string()))
    }
}

async fn start_tunnel_if_needed(
    tunnel: &TunnelMode,
) -> Result<(Option<String>, Option<u16>, Option<CloudflareTunnel>), CliError> {
    match tunnel {
        TunnelMode::Auto => {
            let port = get_available_tcp_port(None)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            let mut cf = CloudflareTunnel::new(port);
            cf.start()
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            let url = cf
                .get_url()
                .ok_or_else(|| CliError::abort("Cloudflare tunnel started but URL was empty"))?;
            Ok((Some(url), Some(port), Some(cf)))
        }
        _ => Ok((None, None, None)),
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
