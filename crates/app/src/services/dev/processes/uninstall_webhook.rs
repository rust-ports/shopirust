//! Send APP_UNINSTALLED webhook when the remote app was updated.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::loader::WebInstance;
use crate::services::webhook::{
    send_app_uninstalled_webhook, send_uninstall_webhook_to_app_server,
    SendUninstallWebhookOptions, WebhookSampleClient,
};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct UninstallWebhookOptions {
    pub store_fqdn: String,
    pub api_secret: String,
    pub remote_app_updated: bool,
    pub backend_port: u16,
    pub frontend_port: u16,
    pub webs: Vec<WebInstance>,
    /// Live sample client from `app dev`. Synthetic payload is used when this is `None`
    /// or when the live fetch fails.
    pub sample_client: Option<Arc<dyn WebhookSampleClient>>,
}

pub fn front_and_backend(webs: &[WebInstance]) -> (Option<&WebInstance>, Option<&WebInstance>) {
    let backend = webs
        .iter()
        .find(|w| w.roles.iter().any(|r| r.eq_ignore_ascii_case("backend")));
    let frontend = webs
        .iter()
        .find(|w| w.roles.iter().any(|r| r.eq_ignore_ascii_case("frontend")));
    (frontend, backend)
}

pub fn setup_send_uninstall_webhook_process(opts: UninstallWebhookOptions) -> Option<DevProcess> {
    let (frontend, backend) = front_and_backend(&opts.webs);
    let webhooks_path = opts
        .webs
        .iter()
        .filter_map(|w| w.webhooks_path.clone())
        .find(|p| !p.is_empty())
        .unwrap_or_else(|| "/api/webhooks".into());
    let has_web = frontend.is_some() || backend.is_some();
    if !opts.remote_app_updated || !has_web {
        return None;
    }
    let delivery_port = if backend.is_some() {
        opts.backend_port
    } else {
        opts.frontend_port
    };
    Some(DevProcess::new(
        "webhooks",
        DevProcessKind::UninstallWebhook,
        move |ctx| run_uninstall(ctx.abort, opts, webhooks_path, delivery_port),
    ))
}

async fn run_uninstall(
    abort: CancellationToken,
    opts: UninstallWebhookOptions,
    webhooks_path: String,
    delivery_port: u16,
) -> Result<(), AppError> {
    let address = format!("http://localhost:{delivery_port}{webhooks_path}");
    tracing::info!(
        target: "app_dev",
        "Sending APP_UNINSTALLED webhook to {address} (store={})",
        opts.store_fqdn
    );

    let send_opts = SendUninstallWebhookOptions {
        address: address.clone(),
        store_fqdn: opts.store_fqdn.clone(),
        shared_secret: opts.api_secret.clone(),
        initial_delay: Duration::from_secs(3),
        retry_delay: Duration::from_secs(5),
        max_attempts: 3,
    };

    let live_ok = if let Some(client) = &opts.sample_client {
        tokio::select! {
            _ = abort.cancelled() => return Ok(()),
            result = send_uninstall_webhook_to_app_server(send_opts.clone(), client.as_ref()) => {
                match result {
                    Ok(true) => true,
                    Ok(false) => {
                        tracing::info!(target: "app_dev", "Live uninstall sample did not deliver; falling back to synthetic payload");
                        false
                    }
                    Err(e) => {
                        tracing::info!(target: "app_dev", "Live uninstall sample failed ({e}); falling back to synthetic payload");
                        false
                    }
                }
            }
        }
    } else {
        false
    };
    if live_ok {
        tracing::info!(target: "app_dev", "APP_UNINSTALLED webhook delivered");
        return Ok(());
    }

    if opts.sample_client.is_none() && send_opts.initial_delay > Duration::ZERO {
        tokio::select! {
            _ = abort.cancelled() => return Ok(()),
            _ = tokio::time::sleep(send_opts.initial_delay) => {}
        }
    }

    let mut last_err = None;
    for attempt in 0..send_opts.max_attempts {
        if abort.is_cancelled() {
            return Ok(());
        }
        match send_app_uninstalled_webhook(&address, &opts.store_fqdn, &opts.api_secret).await {
            Ok(true) => {
                tracing::info!(target: "app_dev", "APP_UNINSTALLED webhook delivered");
                return Ok(());
            }
            Ok(false) => {
                last_err = Some("HTTP delivery failed".into());
                break;
            }
            Err(e) => {
                last_err = Some(e.to_string());
                if attempt + 1 < send_opts.max_attempts {
                    tracing::info!(
                        target: "app_dev",
                        "App isn't responding yet, retrying in {} seconds",
                        send_opts.retry_delay.as_secs()
                    );
                    tokio::select! {
                        _ = abort.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(send_opts.retry_delay) => {}
                    }
                }
            }
        }
    }
    tracing::warn!(
        target: "app_dev",
        "APP_UNINSTALLED webhook delivery failed: {}",
        last_err.unwrap_or_default()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::WebInstance;
    use std::path::PathBuf;

    fn web(role: &str, webhooks: Option<&str>) -> WebInstance {
        WebInstance {
            directory: PathBuf::from("/app/web"),
            configuration_path: PathBuf::from("/app/web/shopify.web.toml"),
            roles: vec![role.into()],
            name: Some(role.into()),
            auth_callback_path: vec![],
            webhooks_path: webhooks.map(str::to_string),
            port: None,
            commands: Default::default(),
            hmr_server: false,
        }
    }

    fn opts(updated: bool, webs: Vec<WebInstance>) -> UninstallWebhookOptions {
        UninstallWebhookOptions {
            store_fqdn: "shop.myshopify.com".into(),
            api_secret: "sec".into(),
            remote_app_updated: updated,
            backend_port: 3457,
            frontend_port: 3000,
            webs,
            sample_client: None,
        }
    }

    #[test]
    fn skips_when_remote_not_updated() {
        let proc =
            setup_send_uninstall_webhook_process(opts(false, vec![web("backend", Some("/hooks"))]));
        assert!(proc.is_none());
    }

    #[test]
    fn skips_without_web_processes() {
        let proc = setup_send_uninstall_webhook_process(opts(true, vec![]));
        assert!(proc.is_none());
    }

    #[test]
    fn selects_when_updated_and_web_present() {
        let proc = setup_send_uninstall_webhook_process(opts(true, vec![web("frontend", None)]));
        assert!(proc.is_some());
    }
}
