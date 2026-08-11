//! Theme app extension host process — optional stub.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ThemeAppExtensionOptions {
    pub store_fqdn: String,
    pub theme: Option<String>,
    pub theme_extension_port: Option<u16>,
    pub extensions: Vec<ExtensionInstance>,
}

pub fn setup_preview_theme_app_extensions_process(
    opts: ThemeAppExtensionOptions,
) -> Option<DevProcess> {
    let has_theme = opts
        .extensions
        .iter()
        .any(|e| e.type_name() == "theme" || e.type_name() == "theme_app_extension");
    if !has_theme {
        return None;
    }

    Some(DevProcess::new(
        "theme-extensions",
        DevProcessKind::ThemeAppExtension,
        move |ctx| run_theme_ext(ctx.abort, opts),
    ))
}

async fn run_theme_ext(
    abort: CancellationToken,
    opts: ThemeAppExtensionOptions,
) -> Result<(), AppError> {
    tracing::info!(
        target: "app_dev",
        "theme app extension stub (store={}, theme={:?}, port={:?})",
        opts.store_fqdn,
        opts.theme,
        opts.theme_extension_port
    );
    abort.cancelled().await;
    Ok(())
}
