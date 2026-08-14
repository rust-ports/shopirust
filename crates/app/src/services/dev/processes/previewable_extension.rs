//! Previewable (UI) extension process — wraps `dev_ui_extensions`.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::app_events::AppEventWatcher;
use crate::services::dev::extension::{dev_ui_extensions, ExtensionDevOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct PreviewableExtensionOptions {
    pub extensions: Vec<ExtensionInstance>,
    pub store_fqdn: String,
    pub store_id: String,
    pub api_key: String,
    pub proxy_url: String,
    pub port: u16,
    pub app_name: String,
    pub app_id: Option<String>,
    pub app_directory: PathBuf,
    pub granted_scopes: Vec<String>,
    pub checkout_cart_url: Option<String>,
    pub subscription_product_url: Option<String>,
    pub build_directory: Option<PathBuf>,
    pub admin_graphql_url: Option<String>,
    pub admin_access_token: Option<String>,
}

pub fn setup_previewable_extensions_process(
    opts: PreviewableExtensionOptions,
    app_watcher: Arc<AppEventWatcher>,
) -> Option<DevProcess> {
    let previewable: Vec<_> = opts
        .extensions
        .iter()
        .filter(|e| e.is_previewable())
        .cloned()
        .collect();
    if previewable.is_empty() {
        return None;
    }

    Some(DevProcess::new(
        "extensions",
        DevProcessKind::PreviewableExtension,
        move |ctx| run_previewable(ctx.abort, opts, previewable, app_watcher),
    ))
}

async fn run_previewable(
    abort: CancellationToken,
    opts: PreviewableExtensionOptions,
    extensions: Vec<ExtensionInstance>,
    app_watcher: Arc<AppEventWatcher>,
) -> Result<(), AppError> {
    let needs_cart = extensions.iter().any(|e| e.should_fetch_cart_url());
    let checkout_cart_url = crate::utilities::fetch_product_variant::build_cart_url_if_needed(
        needs_cart,
        opts.checkout_cart_url.clone(),
        opts.admin_graphql_url.as_deref(),
        opts.admin_access_token.as_deref(),
        &opts.store_fqdn,
    )
    .await
    .ok()
    .flatten()
    .or(opts.checkout_cart_url.clone());
    let mut opts = opts;
    opts.checkout_cart_url = checkout_cart_url;
    let dev_opts = ExtensionDevOptions {
        extensions,
        id: opts.app_id,
        app_name: opts.app_name,
        app_directory: opts.app_directory,
        api_key: opts.api_key,
        url: opts.proxy_url,
        port: opts.port,
        store_fqdn: opts.store_fqdn,
        store_id: opts.store_id,
        granted_scopes: opts.granted_scopes,
        checkout_cart_url: opts.checkout_cart_url,
        subscription_product_url: opts.subscription_product_url,
        manifest_version: "3".into(),
        build_directory: opts.build_directory,
    };
    dev_ui_extensions(dev_opts, app_watcher, abort).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::models::loader::LoadedApp;
    use std::collections::HashMap;

    fn watcher() -> Arc<AppEventWatcher> {
        Arc::new(AppEventWatcher::new(LoadedApp {
            directory: PathBuf::from("/tmp"),
            configuration_path: PathBuf::from("/tmp/shopify.app.toml"),
            configuration: Default::default(),
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: crate::models::identifiers::Identifiers::new(),
            name: "t".into(),
            errors: vec![],
            dev_application_urls: None,
        }))
    }

    fn opts(extensions: Vec<ExtensionInstance>) -> PreviewableExtensionOptions {
        PreviewableExtensionOptions {
            extensions,
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "1".into(),
            api_key: "k".into(),
            proxy_url: "https://example.com".into(),
            port: 9293,
            app_name: "app".into(),
            app_id: Some("1".into()),
            app_directory: PathBuf::from("/tmp"),
            granted_scopes: vec![],
            checkout_cart_url: None,
            subscription_product_url: None,
            build_directory: None,
            admin_graphql_url: None,
            admin_access_token: None,
        }
    }

    #[test]
    fn skips_without_previewable() {
        let spec = create_extension_specification("function").unwrap();
        let ext = ExtensionInstance::new(
            "fn",
            PathBuf::from("/e"),
            PathBuf::from("/e/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        assert!(setup_previewable_extensions_process(opts(vec![ext]), watcher()).is_none());
    }

    #[test]
    fn selects_ui_extension() {
        let spec = create_extension_specification("ui_extension").unwrap();
        let ext = ExtensionInstance::new(
            "ui",
            PathBuf::from("/e"),
            PathBuf::from("/e/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let proc = setup_previewable_extensions_process(opts(vec![ext]), watcher());
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().kind, DevProcessKind::PreviewableExtension);
    }
}
