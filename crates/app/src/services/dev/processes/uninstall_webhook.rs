//! Send APP_UNINSTALLED webhook when the remote app was updated.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::loader::WebInstance;
use crate::services::webhook::{
    deliver_webhook_http, resolve_sample_payload, DeliverWebhookOptions,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct UninstallWebhookOptions {
    pub store_fqdn: String,
    pub api_secret: String,
    pub remote_app_updated: bool,
    pub backend_port: u16,
    pub frontend_port: u16,
    pub webs: Vec<WebInstance>,
}

pub fn front_and_backend<'a>(
    webs: &'a [WebInstance],
) -> (Option<&'a WebInstance>, Option<&'a WebInstance>) {
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
    let sample = resolve_sample_payload("app/uninstalled", "2024-10");
    let mut headers: serde_json::Value =
        serde_json::from_str(&sample.headers).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = headers.as_object_mut() {
        obj.insert(
            "X-Shopify-Shop-Domain".into(),
            serde_json::Value::String(opts.store_fqdn.clone()),
        );
        obj.insert(
            "X-Shopify-Topic".into(),
            serde_json::Value::String("app/uninstalled".into()),
        );
    }
    let headers_json = headers.to_string();

    // Wait briefly so the app server can come up, then retry on connection refused.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let mut last_err = None;
    for attempt in 0..3 {
        if abort.is_cancelled() {
            return Ok(());
        }
        match deliver_webhook_http(DeliverWebhookOptions {
            address: address.clone(),
            body: sample.sample_payload.clone(),
            headers_json: headers_json.clone(),
            shared_secret: Some(opts.api_secret.clone()),
        })
        .await
        {
            Ok(result) if result.success => {
                tracing::info!(target: "app_dev", "APP_UNINSTALLED webhook delivered");
                return Ok(());
            }
            Ok(result) => {
                last_err = Some(format!("HTTP {}", result.status.unwrap_or(0)));
            }
            Err(e) => {
                last_err = Some(e.to_string());
                if attempt < 2 {
                    tracing::info!(
                        target: "app_dev",
                        "App isn't responding yet, retrying in 2 seconds"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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
        }
    }

    #[test]
    fn skips_when_remote_not_updated() {
        let proc = setup_send_uninstall_webhook_process(UninstallWebhookOptions {
            store_fqdn: "shop.myshopify.com".into(),
            api_secret: "sec".into(),
            remote_app_updated: false,
            backend_port: 3457,
            frontend_port: 3000,
            webs: vec![web("backend", Some("/hooks"))],
        });
        assert!(proc.is_none());
    }

    #[test]
    fn skips_without_web_processes() {
        let proc = setup_send_uninstall_webhook_process(UninstallWebhookOptions {
            store_fqdn: "shop.myshopify.com".into(),
            api_secret: "sec".into(),
            remote_app_updated: true,
            backend_port: 3457,
            frontend_port: 3000,
            webs: vec![],
        });
        assert!(proc.is_none());
    }

    #[test]
    fn selects_when_updated_and_web_present() {
        let proc = setup_send_uninstall_webhook_process(UninstallWebhookOptions {
            store_fqdn: "shop.myshopify.com".into(),
            api_secret: "sec".into(),
            remote_app_updated: true,
            backend_port: 3457,
            frontend_port: 3000,
            webs: vec![web("frontend", None)],
        });
        assert!(proc.is_some());
    }

}
