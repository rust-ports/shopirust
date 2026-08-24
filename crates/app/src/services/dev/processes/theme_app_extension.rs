//! Theme app extension host — delegates to `crates/theme` extension server.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::extension::utilities::build_app_url_for_web;
use theme::local_storage::ThemeLocalStorage;
use theme::theme_ext::{
    build_theme_extension_context, initialize_dev_server_session, run_theme_extension_server,
};
use theme::utilities::host_theme_manager::{find_or_create_host_theme, TokenThemeAdmin};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ThemeAppExtensionOptions {
    pub store_fqdn: String,
    pub theme: Option<String>,
    pub theme_extension_port: Option<u16>,
    pub extensions: Vec<ExtensionInstance>,
    pub admin_access_token: Option<String>,
    pub api_key: String,
    pub api_secret: String,
    pub app_url: String,
    pub admin_graphql_url: Option<String>,
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
    let token = resolve_admin_token(&opts).await?;
    let theme_id = resolve_live_host_theme_id(&opts, &token).await?;
    let storefront_password = theme::local_storage::storefront_password_for_store(&opts.store_fqdn);
    let session = initialize_dev_server_session(
        theme_id,
        &opts.store_fqdn,
        &token,
        storefront_password.as_deref(),
    )
    .await
    .map_err(|error| {
        AppError::message(format!("Unable to initialize storefront session: {error}"))
    })?;

    print_theme_ext_next_steps(&opts, theme_id, port);

    let mut ctx = build_theme_extension_context(extension.directory.clone(), theme_id, Some(port));
    ctx.session = session;
    let handle = run_theme_extension_server(ctx)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    abort.cancelled().await;
    handle.close().await;
    Ok(())
}

async fn resolve_admin_token(opts: &ThemeAppExtensionOptions) -> Result<String, AppError> {
    if let Some(token) = opts
        .admin_access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(token.to_string());
    }
    if opts.api_key.is_empty() || opts.api_secret.is_empty() {
        return Err(AppError::message(
            "Unable to authenticate theme app extension preview: app credentials are missing.",
        ));
    }
    mint_admin_token(&opts.store_fqdn, &opts.api_key, &opts.api_secret).await
}

async fn mint_admin_token(
    store_fqdn: &str,
    api_key: &str,
    api_secret: &str,
) -> Result<String, AppError> {
    let url = format!("https://{store_fqdn}/admin/oauth/access_token");
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "client_id": api_key,
            "client_secret": api_secret,
            "grant_type": "client_credentials",
        }))
        .send()
        .await
        .map_err(|error| {
            AppError::message(format!("Unable to authenticate with {store_fqdn}: {error}"))
        })?;
    if !response.status().is_success() {
        return Err(AppError::message(format!(
            "Unable to authenticate with {store_fqdn}: token endpoint returned {}.",
            response.status()
        )));
    }
    let json: serde_json::Value = response.json().await.map_err(|error| {
        AppError::message(format!("Invalid token response from {store_fqdn}: {error}"))
    })?;
    json.get("access_token")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            AppError::message(format!(
                "Token response from {store_fqdn} did not include an access token."
            ))
        })
}

async fn resolve_live_host_theme_id(
    opts: &ThemeAppExtensionOptions,
    token: &str,
) -> Result<i64, AppError> {
    let mut admin = TokenThemeAdmin::new(&opts.store_fqdn, token);
    if let Some(url) = &opts.admin_graphql_url {
        admin = admin.with_graphql_url(url.clone());
    }
    find_or_create_host_theme(
        admin,
        &opts.store_fqdn,
        opts.theme.as_deref(),
        ThemeLocalStorage::new(),
    )
    .await
    .map(|theme| theme.id)
    .map_err(|error| {
        AppError::message(format!(
            "Could not find or create a host theme for theme app extensions: {error}"
        ))
    })
}

pub fn theme_ext_next_steps(
    app_url: &str,
    store_fqdn: &str,
    theme_id: i64,
    port: u16,
) -> Vec<String> {
    vec![
        format!("Install your app in your development store: {app_url}"),
        format!(
            "Setup your theme app extension in the host theme: https://{store_fqdn}/admin/themes/{theme_id}/editor"
        ),
        format!("Preview your theme app extension at http://127.0.0.1:{port}"),
    ]
}

fn print_theme_ext_next_steps(opts: &ThemeAppExtensionOptions, theme_id: i64, port: u16) {
    let app_url = if opts.app_url.is_empty() {
        build_app_url_for_web(&opts.store_fqdn, &opts.api_key)
    } else {
        opts.app_url.clone()
    };
    tracing::info!(
        target: "app_dev",
        "The theme app extension development server is ready."
    );
    for (index, step) in theme_ext_next_steps(&app_url, &opts.store_fqdn, theme_id, port)
        .iter()
        .enumerate()
    {
        tracing::info!(target: "app_dev", "{}. {step}", index + 1);
        println!("{}. {step}", index + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn options(extensions: Vec<ExtensionInstance>) -> ThemeAppExtensionOptions {
        ThemeAppExtensionOptions {
            store_fqdn: "shop.myshopify.com".into(),
            theme: None,
            theme_extension_port: None,
            extensions,
            admin_access_token: None,
            api_key: "key".into(),
            api_secret: "sec".into(),
            app_url: "https://partners.shopify.com/org/apps/1/test".into(),
            admin_graphql_url: None,
        }
    }

    #[test]
    fn skips_without_theme_extension() {
        let proc = setup_preview_theme_app_extensions_process(options(vec![]));
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
        let mut opts = options(vec![ext]);
        opts.theme = Some("123".into());
        opts.theme_extension_port = Some(9293);
        let proc = setup_preview_theme_app_extensions_process(opts);
        assert!(proc.is_some());
    }

    #[test]
    fn next_steps_include_install_editor_and_preview() {
        let steps = theme_ext_next_steps(
            "https://example.com/install",
            "shop.myshopify.com",
            99,
            9293,
        );
        assert!(steps[0].contains("https://example.com/install"));
        assert!(steps[1].contains("https://shop.myshopify.com/admin/themes/99/editor"));
        assert!(steps[2].contains("http://127.0.0.1:9293"));
    }
}
