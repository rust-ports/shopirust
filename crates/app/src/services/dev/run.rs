//! `shopify app dev` orchestrator (T7).

use crate::error::AppError;
use crate::services::context::LinkedAppContext;
use crate::services::dev::port_warnings::{render_port_warnings, PortDetail, PortKind};
use crate::services::dev::processes::{
    run_app_logs_polling, setup_dev_processes, AppLogsPollingOptions, DevNetworkOptions,
    DevProcessContext, SetupDevProcessFlags,
};
use crate::services::dev::tunnel_mode::{
    get_available_tcp_port, TunnelMode, DEFAULT_GRAPHIQL_PORT,
};
use crate::services::dev::urls::{
    generate_application_urls, generate_frontend_url, FrontendUrlOptions,
};
use cli_api::{DeveloperPlatformClient, OrganizationStore};
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
    let _ = options.skip_dependencies_installation;
    let _ = options.notify;
    let _ = options.update;

    let graphiql_requested = options.graphiql_port.unwrap_or(DEFAULT_GRAPHIQL_PORT);
    let graphiql_port = get_available_tcp_port(Some(graphiql_requested)).await?;

    let mut port_details = vec![PortDetail {
        kind: PortKind::Graphiql,
        requested: graphiql_requested,
        actual: graphiql_port,
    }];

    let (proxy_url, proxy_port, using_localhost) = resolve_network(&options).await?;

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

    let frontend = generate_frontend_url(if using_localhost {
        FrontendUrlOptions::Localhost { port: proxy_port }
    } else if let Some(ref _override_url) = options.tunnel_url_override {
        // Custom/auto already absolute URL without `:port` suffix sometimes
        FrontendUrlOptions::Resolved {
            frontend_url: proxy_url.clone(),
            frontend_port: proxy_port,
        }
    } else {
        FrontendUrlOptions::TunnelUrl {
            tunnel_url: format!("{proxy_url}:{proxy_port}"),
        }
    })
    .unwrap_or_else(|_| crate::services::dev::urls::FrontendUrlResult {
        frontend_url: proxy_url.clone(),
        frontend_port: proxy_port,
        using_localhost,
    });
    let _ = frontend;

    let current_urls = generate_application_urls(&proxy_url, None, None);
    let frontend_port = get_available_tcp_port(Some(3000)).await.unwrap_or(3000);
    let backend_port = get_available_tcp_port(Some(3457)).await.unwrap_or(3457);

    let network = DevNetworkOptions {
        proxy_port,
        proxy_url: proxy_url.clone(),
        frontend_port,
        backend_port,
        using_localhost,
        current_urls,
    };

    let setup = setup_dev_processes(
        ctx.app.clone(),
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
            remote_app_updated: false,
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

    // Run logs polling alongside process tasks (client is not `'static`).
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
        // Skip idle app-logs stub — real polling is `logs_fut`.
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
            // Upstream custom format: "https://host:port"
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
    // https://host:123 → https://host ; leave https://host alone
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
