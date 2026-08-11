//! Send APP_UNINSTALLED webhook when remote app was updated — stub.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct UninstallWebhookOptions {
    pub store_fqdn: String,
    pub api_secret: String,
    pub remote_app_updated: bool,
    pub backend_port: u16,
}

pub fn setup_send_uninstall_webhook_process(opts: UninstallWebhookOptions) -> Option<DevProcess> {
    if !opts.remote_app_updated {
        return None;
    }
    Some(DevProcess::new(
        "webhooks",
        DevProcessKind::UninstallWebhook,
        move |ctx| run_uninstall(ctx.abort, opts),
    ))
}

async fn run_uninstall(
    abort: CancellationToken,
    opts: UninstallWebhookOptions,
) -> Result<(), AppError> {
    tracing::info!(
        target: "app_dev",
        "uninstall webhook stub → http://localhost:{} (store={})",
        opts.backend_port,
        opts.store_fqdn
    );
    let _ = opts.api_secret;
    // One-shot in upstream; here we just acknowledge and exit.
    let _ = abort;
    Ok(())
}
