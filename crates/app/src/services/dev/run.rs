//! `shopify app dev` orchestrator (T7).

use crate::error::AppError;
use crate::local_storage::{get_cached_app_info, set_cached_app_info, CachedAppInfo};
use crate::prompts::Prompter;
use crate::services::context::LinkedAppContext;
use crate::services::dependencies::install_app_dependencies;
use crate::services::dev::mkcert::{generate_certificate, MkcertPlatform};
use crate::services::dev::notify::DevNotifier;
use crate::services::dev::port_warnings::{render_port_warnings, PortDetail, PortKind};
use crate::services::dev::processes::{
    run_app_logs_polling, setup_dev_processes, AppLogsPollingOptions, DevNetworkOptions,
    DevProcessContext, SetupDevProcessFlags,
};
use crate::services::dev::tunnel_mode::{
    get_available_tcp_port, TunnelMode, DEFAULT_GRAPHIQL_PORT,
};
use crate::services::dev::urls::{
    auth_callback_paths_from_webs, generate_application_urls, generate_frontend_url, get_urls,
    proxy_url_from_frontend, should_or_prompt_update_urls, update_urls, FrontendUrlOptions,
    ShouldUpdateUrlsOptions,
};
use cli_api::{DeveloperPlatformClient, OrganizationStore};
use serde_json::json;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct DevOptions {
    pub directory: PathBuf,
    pub update: bool,
    pub skip_dependencies_installation: bool,
    pub subscription_product_url: Option<String>,
    pub checkout_cart_url: Option<String>,
    pub tunnel: TunnelMode,
    /// When `TunnelMode::Auto`, command starts tunnel and fills this URL.
    pub tunnel_url_override: Option<String>,
    pub tunnel_local_port: Option<u16>,
    pub theme: Option<String>,
    pub theme_extension_port: Option<u16>,
    pub notify: Option<String>,
    pub graphiql_port: Option<u16>,
    pub graphiql_key: Option<String>,
    pub app_dev_token: String,
    pub app_dev_graphql_url: String,
}

/// Run `app dev`: prepare network → setup processes → run until Ctrl+C.
pub async fn dev(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    store: &OrganizationStore,
    options: DevOptions,
) -> Result<(), AppError> {
    dev_with_prompter(ctx, client, store, options, None).await
}

pub async fn dev_with_prompter(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    store: &OrganizationStore,
    options: DevOptions,
    prompter: Option<&dyn Prompter>,
) -> Result<(), AppError> {
    if !options.skip_dependencies_installation {
        let _ = install_app_dependencies(&options.directory, false, None);
    }

    let graphiql_requested = options.graphiql_port.unwrap_or(DEFAULT_GRAPHIQL_PORT);
    let graphiql_port = get_available_tcp_port(Some(graphiql_requested)).await?;

    let mut port_details = vec![PortDetail {
        kind: PortKind::Graphiql,
        requested: graphiql_requested,
        actual: graphiql_port,
    }];

    let (frontend_url, proxy_port, using_localhost) = resolve_network(&options).await?;
    let frontend = generate_frontend_url(if using_localhost {
        FrontendUrlOptions::Localhost { port: proxy_port }
    } else {
        FrontendUrlOptions::Resolved {
            frontend_url: frontend_url.clone(),
            frontend_port: proxy_port,
        }
    })
    .unwrap_or_else(|_| crate::services::dev::urls::FrontendUrlResult {
        frontend_url: frontend_url.clone(),
        frontend_port: proxy_port,
        using_localhost,
    });
    let proxy_url = if using_localhost {
        proxy_url_from_frontend(&frontend)
    } else {
        frontend.frontend_url.clone()
    };

    if let TunnelMode::UseLocalhost {
        requested_port,
        actual_port,
    } = &options.tunnel
    {
        port_details.push(PortDetail {
            kind: PortKind::Localhost,
            requested: *requested_port,
            actual: *actual_port,
        });
    }

    for warning in render_port_warnings(&port_details) {
        eprintln!("{warning}");
    }

    let reverse_proxy_cert = if using_localhost {
        Some(
            generate_certificate(
                &options.directory,
                prompter,
                &[],
                MkcertPlatform::current().unwrap_or(MkcertPlatform::LinuxAmd64),
                None,
            )
            .await?,
        )
    } else {
        None
    };

    let mut local_app = ctx.app.clone();
    let auth_paths = auth_callback_paths_from_webs(&local_app.webs);
    let proxy_fields = local_app.configuration.extra.get("app_proxy").and_then(|p| {
        Some((
            p.get("url")?.as_str()?.to_string(),
            p.get("subpath")?.as_str()?.to_string(),
            p.get("prefix")?.as_str()?.to_string(),
        ))
    });
    let new_urls = generate_application_urls(
        &proxy_url,
        auth_paths.as_deref(),
        proxy_fields,
    );

    let remote_config = json!({
        "application_url": ctx.remote_app.application_url,
        "auth": { "redirect_urls": ctx.remote_app.redirect_url_whitelist },
    });
    let current_urls = get_urls(Some(&remote_config));

    let cached = get_cached_app_info(&options.directory);
    let cached_update = local_app
        .configuration
        .build
        .as_ref()
        .and_then(|b| b.automatically_update_urls_on_dev)
        .or_else(|| cached.as_ref().and_then(|c| c.update_urls));
    let previous_app_id = cached.as_ref().and_then(|c| c.previous_app_id.clone());
    let remote_app_updated = previous_app_id.as_deref() != Some(ctx.remote_app.api_key.as_str());

    let partner_urls_updated = if options.update {
        let should = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: current_urls.clone(),
                app_directory: &options.directory,
                cached_update_urls: cached_update,
                new_app: false,
                local_app: Some(&local_app),
                api_key: ctx.remote_app.api_key.clone(),
                new_urls: new_urls.clone(),
                using_dev_sessions: client.supports_dev_sessions(),
                interactive: prompter.is_some() && is_terminal::is_terminal(std::io::stdin()),
            },
            prompter,
        )?;
        if should {
            if client.supports_dev_sessions() {
                local_app.set_dev_application_urls(new_urls.clone());
            } else {
                update_urls(
                    &new_urls,
                    &ctx.remote_app.api_key,
                    client,
                    Some(&local_app),
                )
                .await?;
            }
        }
        should
    } else {
        false
    };
    let _ = partner_urls_updated;

    let _ = set_cached_app_info(&CachedAppInfo {
        directory: options.directory.display().to_string(),
        previous_app_id: Some(ctx.remote_app.api_key.clone()),
        ..cached.unwrap_or_default()
    });

    let frontend_port = get_available_tcp_port(Some(3000)).await.unwrap_or(3000);
    let backend_port = get_available_tcp_port(Some(3457)).await.unwrap_or(3457);

    let network = DevNetworkOptions {
        proxy_port,
        proxy_url: proxy_url.clone(),
        frontend_port,
        backend_port,
        using_localhost,
        current_urls: new_urls,
        reverse_proxy_cert,
    };

    let setup = setup_dev_processes(
        local_app,
        &ctx.remote_app,
        &store.shop_domain,
        &store.shop_id,
        &network,
        SetupDevProcessFlags {
            subscription_product_url: options.subscription_product_url.clone(),
            checkout_cart_url: options.checkout_cart_url.clone(),
            theme: options.theme.clone(),
            theme_extension_port: options.theme_extension_port,
            graphiql_port,
            graphiql_key: options.graphiql_key.clone(),
            enable_graphiql: std::env::var("SHOPIFY_CLI_DISABLE_GRAPHIQL")
                .ok()
                .as_deref()
                != Some("1"),
            supports_dev_sessions: client.supports_dev_sessions(),
            remote_app_updated,
            app_dev_token: options.app_dev_token.clone(),
            app_dev_graphql_url: options.app_dev_graphql_url.clone(),
        },
    )
    .await;

    println!("Preview URL: {}", setup.preview_url);
    if let Some(ref gurl) = setup.graphiql_url {
        println!("GraphiQL URL: {gurl}");
    }
    println!("Press Ctrl+C to stop\n");

    let cancel = CancellationToken::new();
    let cancel_ctrlc = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancel_ctrlc.cancel();
    });

    let shop_id_parsed = store.shop_id.parse::<i64>().ok();
    let logs_fut = async {
        if let Some(shop_id) = shop_id_parsed {
            let opts = AppLogsPollingOptions {
                organization_id: ctx.organization.id.clone(),
                api_key: ctx.remote_app.api_key.clone(),
                shop_ids: vec![shop_id],
                store_name: store.shop_domain.clone(),
            };
            let _ = run_app_logs_polling(cancel.clone(), client, opts).await;
        } else {
            cancel.cancelled().await;
        }
    };

    let mut handles = Vec::new();
    for proc in setup.processes {
        if proc.kind == crate::services::dev::processes::DevProcessKind::AppLogsPolling {
            continue;
        }
        let prefix = proc.prefix.clone();
        let ctx_proc = DevProcessContext {
            abort: cancel.clone(),
            prefix: prefix.clone(),
        };
        let run = proc.run;
        handles.push(tokio::spawn(async move {
            if let Err(e) = run(ctx_proc).await {
                eprintln!("[{prefix}] error: {e}");
            }
        }));
    }

    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = logs_fut => {}
    }

    if let Some(ref target) = options.notify {
        let _ = DevNotifier::new(target.clone()).notify("idle").await;
    }

    for h in handles {
        let _ = h.await;
    }

    println!("\nStopped app dev.");
    Ok(())
}

async fn resolve_network(options: &DevOptions) -> Result<(String, u16, bool), AppError> {
    match &options.tunnel {
        TunnelMode::Auto => {
            let url = options.tunnel_url_override.clone().ok_or_else(|| {
                AppError::message(
                    "Auto tunnel URL missing. Ensure cloudflared is installed, or use --use-localhost / --tunnel-url.",
                )
            })?;
            let port = options
                .tunnel_local_port
                .unwrap_or_else(|| parse_port_from_url(&url).unwrap_or(443));
            Ok((strip_port(&url), port, false))
        }
        TunnelMode::Custom { url } => {
            if let Ok(parsed) = generate_frontend_url(FrontendUrlOptions::TunnelUrl {
                tunnel_url: url.clone(),
            }) {
                Ok((parsed.frontend_url, parsed.frontend_port, false))
            } else {
                Ok((
                    strip_port(url),
                    parse_port_from_url(url).unwrap_or(443),
                    false,
                ))
            }
        }
        TunnelMode::UseLocalhost { actual_port, .. } => {
            Ok(("https://localhost".to_string(), *actual_port, true))
        }
    }
}

fn strip_port(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after = &url[scheme_end + 3..];
        if let Some(colon) = after.rfind(':') {
            let maybe_port = &after[colon + 1..];
            if maybe_port.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}{}", &url[..scheme_end + 3], &after[..colon]);
            }
        }
    }
    url.to_string()
}

fn parse_port_from_url(url: &str) -> Option<u16> {
    let after = url.split("://").nth(1)?;
    let port = after.rsplit(':').next()?;
    port.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dev::mkcert::LocalhostCert;

    #[test]
    fn strip_and_parse_port() {
        assert_eq!(
            strip_port("https://example.trycloudflare.com:4040"),
            "https://example.trycloudflare.com"
        );
        assert_eq!(
            parse_port_from_url("https://example.trycloudflare.com:4040"),
            Some(4040)
        );
    }

    #[test]
    fn localhost_cert_type_exists() {
        let _ = LocalhostCert {
            key: "k".into(),
            cert: "c".into(),
            cert_path: ".shopify/localhost.pem".into(),
        };
    }
}
