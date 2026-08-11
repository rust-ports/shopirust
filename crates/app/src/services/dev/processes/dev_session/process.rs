//! Dev-session concurrent process.

use super::super::types::{DevProcess, DevProcessKind};
use super::DevSessionClient;
use crate::error::AppError;
use crate::services::dev::app_events::AppEventWatcher;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct DevSessionProcessOptions {
    pub client: DevSessionClient,
    pub app_id: String,
    pub assets_url: String,
    pub websocket_url: Option<String>,
    pub app_preview_url: String,
}

pub fn setup_dev_session_process(
    opts: DevSessionProcessOptions,
    _app_watcher: Arc<AppEventWatcher>,
) -> DevProcess {
    DevProcess::new("app-preview", DevProcessKind::DevSession, move |ctx| {
        run_dev_session(ctx.abort, opts)
    })
}

async fn run_dev_session(
    abort: CancellationToken,
    opts: DevSessionProcessOptions,
) -> Result<(), AppError> {
    match opts
        .client
        .create(
            &opts.app_id,
            &opts.assets_url,
            opts.websocket_url.as_deref(),
        )
        .await
    {
        Ok(()) => {
            tracing::info!(
                target: "app_dev",
                "Dev session created. Preview: {}",
                opts.app_preview_url
            );
            println!("Dev session ready — preview: {}", opts.app_preview_url);
        }
        Err(e) => {
            // Soft-fail so local preview still works without Next-Gen API access.
            tracing::warn!(target: "app_dev", "dev session create skipped: {e}");
            println!("Dev session create skipped ({e}). Local preview still running.");
        }
    }

    abort.cancelled().await;

    if let Err(e) = opts.client.delete(&opts.app_id).await {
        tracing::warn!(target: "app_dev", "dev session delete on shutdown: {e}");
    }
    Ok(())
}
