//! Theme app extension host — delegates to `crates/theme` extension server.

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
    let theme_ext = opts
        .extensions
        .iter()
        .find(|e| e.type_name() == "theme" || e.type_name() == "theme_app_extension")
        .cloned()?;

    Some(DevProcess::new(
        "theme-extensions",
        DevProcessKind::ThemeAppExtension,
        move |ctx| run_theme_ext(ctx.abort, opts, theme_ext),
    ))
}

async fn run_theme_ext(
    abort: CancellationToken,
    opts: ThemeAppExtensionOptions,
    extension: ExtensionInstance,
) -> Result<(), AppError> {
    let port = opts
        .theme_extension_port
        .unwrap_or(theme::theme_ext::DEFAULT_THEME_EXT_PORT);
    let theme_id = opts
        .theme
        .as_deref()
        .and_then(|t| t.parse::<i64>().ok())
        .unwrap_or(1);

    tracing::info!(
        target: "app_dev",
        "theme app extension host on http://127.0.0.1:{port} (store={}, theme={:?})",
        opts.store_fqdn,
        opts.theme
    );

    let ctx = theme::theme_ext::build_theme_extension_context(
        extension.directory.clone(),
        theme_id,
        Some(port),
    );
    let handle = theme::theme_ext::run_theme_extension_server(ctx)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    abort.cancelled().await;
    handle.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn skips_without_theme_extension() {
        let proc = setup_preview_theme_app_extensions_process(ThemeAppExtensionOptions {
            store_fqdn: "shop.myshopify.com".into(),
            theme: None,
            theme_extension_port: None,
            extensions: vec![],
        });
        assert!(proc.is_none());
    }

    #[test]
    fn selects_theme_extension() {
        let spec = create_extension_specification("theme").unwrap();
        let ext = ExtensionInstance::new(
            "theme-ext",
            PathBuf::from("/app/extensions/theme"),
            PathBuf::from("/app/extensions/theme/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let proc = setup_preview_theme_app_extensions_process(ThemeAppExtensionOptions {
            store_fqdn: "shop.myshopify.com".into(),
            theme: Some("123".into()),
            theme_extension_port: Some(9293),
            extensions: vec![ext],
        });
        assert!(proc.is_some());
    }
}
