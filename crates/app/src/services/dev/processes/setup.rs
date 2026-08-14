//! Build the concurrent process list for `app dev`.

use super::app_logs_polling::{setup_app_logs_polling_process, AppLogsPollingOptions};
use super::app_watcher::setup_app_watcher_process;
use super::dev_session::{
    setup_dev_session_process, DevSessionClient, DevSessionProcessOptions, DevSessionStatusManager,
};
use super::draftable_extension::{setup_draftable_extensions_process, DraftableExtensionOptions};
use super::graphiql::{setup_graphiql_server_process, GraphiqlOptions};
use super::previewable_extension::{
    setup_previewable_extensions_process, PreviewableExtensionOptions,
};
use super::proxy::{setup_proxy_server_process, ProxyServerOptions};
use super::theme_app_extension::{
    setup_preview_theme_app_extensions_process, ThemeAppExtensionOptions,
};
use super::types::{DevProcess, DevProcessKind};
use super::uninstall_webhook::{setup_send_uninstall_webhook_process, UninstallWebhookOptions};
use super::utils::DevNetworkOptions;
use super::web::setup_web_processes;
use std::collections::BTreeMap;
use crate::models::loader::LoadedApp;
use crate::services::dev::app_events::AppEventWatcher;
use crate::services::dev::extension::get_websocket_url;
use crate::services::dev::tunnel_mode::get_available_tcp_port;
use crate::services::webhook::WebhookSampleClient;
use cli_api::OrganizationApp;
use std::sync::Arc;

#[derive(Clone)]
pub struct SetupDevProcessFlags {
    pub subscription_product_url: Option<String>,
    pub checkout_cart_url: Option<String>,
    pub theme: Option<String>,
    pub theme_extension_port: Option<u16>,
    pub graphiql_port: u16,
    pub graphiql_key: Option<String>,
    pub enable_graphiql: bool,
    pub supports_dev_sessions: bool,
    pub remote_app_updated: bool,
    /// Bearer token for app_dev GraphQL (empty → soft-skip create).
    pub app_dev_token: String,
    pub app_dev_graphql_url: String,
    pub webhook_sample_client: Option<Arc<dyn WebhookSampleClient>>,
    /// Shared platform client for Partners draft push / logs polling.
    pub platform_client: Option<Arc<dyn cli_api::DeveloperPlatformClient>>,
    pub admin_graphql_url: Option<String>,
    pub admin_access_token: Option<String>,
}

pub struct SetupDevProcessesResult {
    pub processes: Vec<DevProcess>,
    pub preview_url: String,
    pub graphiql_url: Option<String>,
    pub app_watcher: Arc<AppEventWatcher>,
    pub status: Arc<DevSessionStatusManager>,
}

/// Select and configure concurrent processes (unit-testable without spawning).
pub async fn setup_dev_processes(
    local_app: LoadedApp,
    remote_app: &OrganizationApp,
    store_fqdn: &str,
    store_id: &str,
    network: &DevNetworkOptions,
    flags: SetupDevProcessFlags,
) -> SetupDevProcessesResult {
    let api_key = remote_app.api_key.clone();
    let api_secret = remote_app
        .api_secret_keys
        .first()
        .map(|k| k.secret.clone())
        .unwrap_or_default();
    let scopes = local_app.configuration.scopes().join(",");

    let app_watcher = Arc::new(AppEventWatcher::new(local_app.clone()));
    let status = Arc::new(DevSessionStatusManager::new());

    let any_previewable = local_app.extensions.iter().any(|e| e.is_previewable());
    let dev_console_url = format!("{}/extensions/dev-console", network.proxy_url);
    let app_preview_url = format!("https://{}/admin/apps/{}", store_fqdn, api_key);
    let preview_url = if any_previewable {
        dev_console_url.clone()
    } else {
        app_preview_url.clone()
    };

    let graphiql_key = flags
        .graphiql_key
        .clone()
        .unwrap_or_else(|| format!("{}:{}", api_secret, store_fqdn));
    let graphiql_url = if flags.enable_graphiql {
        Some(format!(
            "http://localhost:{}/graphiql?key={}",
            flags.graphiql_port,
            urlencoding_lite(&graphiql_key)
        ))
    } else {
        None
    };

    let mut processes: Vec<DevProcess> = Vec::new();

    processes.extend(setup_web_processes(
        &local_app.webs,
        &network.proxy_url,
        network.frontend_port,
        network.backend_port,
        &api_key,
        &api_secret,
        &scopes,
    ));

    if flags.enable_graphiql {
        processes.push(setup_graphiql_server_process(GraphiqlOptions {
            port: flags.graphiql_port,
            app_name: remote_app.title.clone(),
            app_url: app_preview_url.clone(),
            store_fqdn: store_fqdn.to_string(),
            key: graphiql_key,
            api_key: api_key.clone(),
            api_secret: api_secret.clone(),
            graphql_url: None,
        }));
    }

    let ext_port = get_available_tcp_port(None).await.unwrap_or(9293);
    if let Some(p) = setup_previewable_extensions_process(
        PreviewableExtensionOptions {
            extensions: local_app.extensions.clone(),
            store_fqdn: store_fqdn.to_string(),
            store_id: store_id.to_string(),
            api_key: api_key.clone(),
            proxy_url: network.proxy_url.clone(),
            port: ext_port,
            app_name: local_app.name.clone(),
            app_id: Some(remote_app.id.clone()),
            app_directory: local_app.directory.clone(),
            granted_scopes: remote_app.granted_scopes.clone(),
            checkout_cart_url: flags.checkout_cart_url.clone(),
            subscription_product_url: flags.subscription_product_url.clone(),
            build_directory: Some(app_watcher.build_output_path.clone()),
            admin_graphql_url: flags.admin_graphql_url.clone(),
            admin_access_token: flags.admin_access_token.clone(),
        },
        app_watcher.clone(),
    ) {
        processes.push(p);
    }

    if flags.supports_dev_sessions {
        let ws = get_websocket_url(&network.proxy_url);
        processes.push(setup_dev_session_process(
            DevSessionProcessOptions {
                client: DevSessionClient::new(
                    store_fqdn,
                    flags.app_dev_token.clone(),
                    flags.app_dev_graphql_url.clone(),
                ),
                app_id: remote_app.id.clone(),
                assets_url: network.proxy_url.clone(),
                websocket_url: Some(ws),
                app_preview_url: preview_url.clone(),
            },
            app_watcher.clone(),
            status.clone(),
        ));
    } else if let Some(p) = setup_draftable_extensions_process(
        DraftableExtensionOptions {
            api_key: api_key.clone(),
            proxy_url: network.proxy_url.clone(),
            extensions: local_app.extensions.clone(),
            remote_extension_ids: local_app.identifiers.extensions.clone(),
            app_configuration: Some(serde_json::to_value(&local_app.configuration).unwrap_or_default()),
            client: flags.platform_client.clone(),
        },
        app_watcher.clone(),
    ) {
        processes.push(p);
    }

    if let Some(p) = setup_preview_theme_app_extensions_process(ThemeAppExtensionOptions {
        store_fqdn: store_fqdn.to_string(),
        theme: flags.theme.clone(),
        theme_extension_port: flags.theme_extension_port,
        extensions: local_app.extensions.clone(),
    }) {
        processes.push(p);
    }

    if let Some(p) = setup_send_uninstall_webhook_process(UninstallWebhookOptions {
        store_fqdn: store_fqdn.to_string(),
        api_secret: api_secret.clone(),
        remote_app_updated: flags.remote_app_updated,
        backend_port: network.backend_port,
        frontend_port: network.frontend_port,
        webs: local_app.webs.clone(),
        sample_client: flags.webhook_sample_client.clone(),
    }) {
        processes.push(p);
    }

    if let Ok(shop_id) = store_id.parse::<i64>() {
        processes.push(setup_app_logs_polling_process(AppLogsPollingOptions {
            organization_id: remote_app.organization_id.clone().unwrap_or_default(),
            api_key: api_key.clone(),
            client: flags.platform_client.clone(),
            shop_ids: vec![shop_id],
            store_name: store_fqdn.to_string(),
            logs_dir: Some(local_app.directory.join(".shopify").join("logs")),
        }));
    }

    processes.push(setup_app_watcher_process(app_watcher.clone()));

    let mut proxy_rules = BTreeMap::new();
    if local_app
        .webs
        .iter()
        .any(|w| w.roles.iter().any(|r| r.eq_ignore_ascii_case("frontend")))
    {
        proxy_rules.insert(
            "default".into(),
            format!("http://localhost:{}", network.frontend_port),
        );
    }
    if any_previewable {
        proxy_rules.insert(
            "/extensions".into(),
            format!("http://localhost:{ext_port}"),
        );
    }
    if !proxy_rules.is_empty() {
        processes.push(setup_proxy_server_process(ProxyServerOptions {
            port: network.proxy_port,
            rules: proxy_rules,
            localhost_cert: network.reverse_proxy_cert.clone(),
        }));
    }

    SetupDevProcessesResult {
        processes,
        preview_url,
        graphiql_url,
        app_watcher,
        status,
    }
}

/// Process kinds selected for a given configuration (for unit tests).
pub fn selected_process_kinds(processes: &[DevProcess]) -> Vec<DevProcessKind> {
    processes.iter().map(|p| p.kind).collect()
}

fn urlencoding_lite(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::AppConfiguration;
    use crate::models::identifiers::Identifiers;
    use crate::services::dev::urls::ApplicationUrls;
    use cli_api::{ApiSecretKey, OrganizationApp};
    use std::path::PathBuf;

    fn empty_app() -> LoadedApp {
        LoadedApp {
            directory: PathBuf::from("/tmp/app"),
            configuration_path: PathBuf::from("/tmp/app/shopify.app.toml"),
            configuration: AppConfiguration::default(),
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: Identifiers::new(),
            name: "test-app".into(),
            errors: vec![],
            dev_application_urls: None,
        }
    }

    fn remote() -> OrganizationApp {
        OrganizationApp {
            id: "1".into(),
            title: "Test".into(),
            api_key: "key".into(),
            organization_id: Some("org".into()),
            api_secret_keys: vec![ApiSecretKey {
                secret: "sec".into(),
            }],
            granted_scopes: vec!["write_products".into()],
            application_url: None,
            redirect_url_whitelist: vec![],
            flags: vec![],
        }
    }

    fn network() -> DevNetworkOptions {
        DevNetworkOptions {
            proxy_port: 3458,
            proxy_url: "https://example.trycloudflare.com".into(),
            frontend_port: 3000,
            backend_port: 3457,
            using_localhost: false,
            current_urls: ApplicationUrls {
                application_url: "https://example.trycloudflare.com".into(),
                redirect_url_whitelist: vec![],
                app_proxy: None,
            },
            reverse_proxy_cert: None,
        }
    }

    #[tokio::test]
    async fn selects_dev_session_when_supported() {
        let result = setup_dev_processes(
            empty_app(),
            &remote(),
            "shop.myshopify.com",
            "123",
            &network(),
            SetupDevProcessFlags {
                subscription_product_url: None,
                checkout_cart_url: None,
                theme: None,
                theme_extension_port: None,
                graphiql_port: 3457,
                graphiql_key: None,
                enable_graphiql: true,
                supports_dev_sessions: true,
                remote_app_updated: false,
                app_dev_token: String::new(),
                app_dev_graphql_url: "https://example/graphql".into(),
                webhook_sample_client: None,
                platform_client: None,
                admin_graphql_url: None,
                admin_access_token: None,
            },
        )
        .await;
        let kinds = selected_process_kinds(&result.processes);
        assert!(kinds.contains(&DevProcessKind::DevSession));
        assert!(kinds.contains(&DevProcessKind::Graphiql));
        assert!(kinds.contains(&DevProcessKind::AppWatcher));
        assert!(!kinds.contains(&DevProcessKind::DraftableExtension));
        assert!(result.graphiql_url.is_some());
    }

    #[tokio::test]
    async fn selects_draftable_when_no_dev_sessions() {
        let mut app = empty_app();
        let spec = crate::models::extensions::create_extension_specification("function").unwrap();
        app.extensions.push(crate::models::extensions::ExtensionInstance::new(
            "discount",
            PathBuf::from("/tmp/app/extensions/discount"),
            PathBuf::from("/tmp/app/extensions/discount/shopify.extension.toml"),
            Default::default(),
            spec,
        ));
        app.webs.push(crate::models::loader::WebInstance {
            directory: PathBuf::from("/tmp/app/web"),
            configuration_path: PathBuf::from("/tmp/app/web/shopify.web.toml"),
            roles: vec!["backend".into()],
            name: Some("web".into()),
            auth_callback_path: vec![],
            webhooks_path: Some("/api/webhooks".into()),
            port: None,
            commands: Default::default(),
            hmr_server: false,
        });
        let result = setup_dev_processes(
            app,
            &remote(),
            "shop.myshopify.com",
            "123",
            &network(),
            SetupDevProcessFlags {
                subscription_product_url: None,
                checkout_cart_url: None,
                theme: None,
                theme_extension_port: None,
                graphiql_port: 3457,
                graphiql_key: None,
                enable_graphiql: false,
                supports_dev_sessions: false,
                remote_app_updated: true,
                app_dev_token: String::new(),
                app_dev_graphql_url: "https://example/graphql".into(),
                webhook_sample_client: None,
                platform_client: None,
                admin_graphql_url: None,
                admin_access_token: None,
            },
        )
        .await;
        let kinds = selected_process_kinds(&result.processes);
        assert!(kinds.contains(&DevProcessKind::DraftableExtension));
        assert!(kinds.contains(&DevProcessKind::UninstallWebhook));
        assert!(!kinds.contains(&DevProcessKind::DevSession));
        assert!(!kinds.contains(&DevProcessKind::Graphiql));
    }

    #[tokio::test]
    async fn selects_proxy_when_frontend_web_present() {
        let mut app = empty_app();
        app.webs.push(crate::models::loader::WebInstance {
            directory: PathBuf::from("/tmp/app/web"),
            configuration_path: PathBuf::from("/tmp/app/web/shopify.web.toml"),
            roles: vec!["frontend".into()],
            name: Some("web".into()),
            auth_callback_path: vec!["/auth/callback".into()],
            webhooks_path: Some("/api/webhooks".into()),
            port: None,
            commands: Default::default(),
            hmr_server: false,
        });
        let result = setup_dev_processes(
            app,
            &remote(),
            "shop.myshopify.com",
            "123",
            &network(),
            SetupDevProcessFlags {
                subscription_product_url: None,
                checkout_cart_url: None,
                theme: None,
                theme_extension_port: None,
                graphiql_port: 3457,
                graphiql_key: None,
                enable_graphiql: false,
                supports_dev_sessions: true,
                remote_app_updated: false,
                app_dev_token: String::new(),
                app_dev_graphql_url: "https://example/graphql".into(),
                webhook_sample_client: None,
                platform_client: None,
                admin_graphql_url: None,
                admin_access_token: None,
            },
        )
        .await;
        let kinds = selected_process_kinds(&result.processes);
        assert!(kinds.contains(&DevProcessKind::ProxyServer));
        assert!(kinds.contains(&DevProcessKind::Web));
    }
}
