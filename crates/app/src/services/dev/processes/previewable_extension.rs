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
        manifest_version: uuid::Uuid::new_v4().to_string(),
        build_directory: opts.build_directory,
    };
    dev_ui_extensions(dev_opts, app_watcher, abort).await
}
