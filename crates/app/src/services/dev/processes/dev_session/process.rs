//! Dev-session concurrent process.

use super::super::types::{DevProcess, DevProcessKind};
use super::status::{inherited_module_uids, DevSessionStatusManager};
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
    app_watcher: Arc<AppEventWatcher>,
    status: Arc<DevSessionStatusManager>,
) -> DevProcess {
    DevProcess::new("app-preview", DevProcessKind::DevSession, move |ctx| {
        run_dev_session(ctx.abort, opts, app_watcher, status)
    })
}

async fn run_dev_session(
    abort: CancellationToken,
    opts: DevSessionProcessOptions,
    watcher: Arc<AppEventWatcher>,
    status: Arc<DevSessionStatusManager>,
) -> Result<(), AppError> {
    status.set_loading();
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
            status.set_ready(vec![]);
            tracing::info!(
                target: "app_dev",
                "Dev session created. Preview: {}",
                opts.app_preview_url
            );
            println!("Dev session ready — preview: {}", opts.app_preview_url);
        }
        Err(e) => {
            status.set_error(e.to_string());
            tracing::warn!(target: "app_dev", "dev session create skipped: {e}");
            println!("Dev session create skipped ({e}). Local preview still running.");
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    watcher
        .on_event(move |event| {
            let _ = tx.send(event);
        })
        .await;

    loop {
        tokio::select! {
            _ = abort.cancelled() => break,
            maybe = rx.recv() => {
                let Some(event) = maybe else { break };
                status.set_loading();
                let inherited = inherited_module_uids(&event);
                match opts
                    .client
                    .update(&opts.app_id, Some(&opts.assets_url), &inherited)
                    .await
                {
                    Ok(()) => {
                        status.apply_event(&event);
                        tracing::info!(target: "app_dev", "devSessionUpdate ok");
                    }
                    Err(e) => {
                        status.set_error(e.to_string());
                        tracing::warn!(target: "app_dev", "devSessionUpdate failed: {e}");
                    }
                }
            }
        }
    }

    if let Err(e) = opts.client.delete(&opts.app_id).await {
        tracing::warn!(target: "app_dev", "dev session delete on shutdown: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::LoadedApp;
    use crate::services::dev::processes::DevProcessKind;
    use std::path::PathBuf;

    #[test]
    fn setup_returns_dev_session_kind() {
        let watcher = Arc::new(AppEventWatcher::new(LoadedApp {
            directory: PathBuf::from("/tmp"),
            configuration_path: PathBuf::from("/tmp/shopify.app.toml"),
            configuration: Default::default(),
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: crate::models::identifiers::Identifiers::new(),
            name: "t".into(),
            errors: vec![],
            dev_application_urls: None,
        }));
        let proc = setup_dev_session_process(
            DevSessionProcessOptions {
                client: DevSessionClient::new("shop.myshopify.com", "", "https://example/graphql"),
                app_id: "1".into(),
                assets_url: "https://example.com".into(),
                websocket_url: None,
                app_preview_url: "https://shop.myshopify.com/admin/apps/1".into(),
            },
            watcher,
            Arc::new(DevSessionStatusManager::new()),
        );
        assert_eq!(proc.kind, DevProcessKind::DevSession);
        assert_eq!(proc.prefix, "app-preview");
    }
}
