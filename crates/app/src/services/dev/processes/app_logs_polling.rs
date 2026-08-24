//! App logs polling during `app dev` — reuses [`AppLogsPoller`].

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::services::app_logs::{
    format_log_text, subscribe_to_app_logs, write_app_logs_to_file, AppLogsPoller, PollBackend,
    PollFilters,
};
use cli_api::DeveloperPlatformClient;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppLogsPollingOptions {
    pub organization_id: String,
    pub api_key: String,
    pub shop_ids: Vec<i64>,
    pub store_name: String,
    /// When set, persist JSON log files under this directory (`.shopify/logs`).
    pub logs_dir: Option<PathBuf>,
    pub client: Option<std::sync::Arc<dyn DeveloperPlatformClient>>,
}

/// Poll app logs when a platform client is available; otherwise idle until cancel.
pub fn setup_app_logs_polling_process(opts: AppLogsPollingOptions) -> DevProcess {
    DevProcess::new(
        "app-logs",
        DevProcessKind::AppLogsPolling,
        move |ctx| async move {
            if let Some(client) = opts.client.clone() {
                run_app_logs_polling(ctx.abort, client.as_ref(), opts).await
            } else {
                ctx.abort.cancelled().await;
                Ok(())
            }
        },
    )
}

/// Run logs polling with a live developer-platform client until cancelled.
pub async fn run_app_logs_polling(
    abort: CancellationToken,
    client: &dyn DeveloperPlatformClient,
    opts: AppLogsPollingOptions,
) -> Result<(), AppError> {
    let jwt =
        subscribe_to_app_logs(client, &opts.shop_ids, &opts.api_key, &opts.organization_id).await?;

    let mut poller = AppLogsPoller::new(jwt, PollFilters::default());
    let backend = PollBackend::Platform {
        client,
        organization_id: opts.organization_id.clone(),
    };
    let store_name = opts.store_name.clone();
    let shop_ids = opts.shop_ids.clone();
    let api_key = opts.api_key.clone();
    let org_id = opts.organization_id.clone();
    let logs_dir = opts.logs_dir.clone();

    let loop_fut = poller.run_loop(
        &backend,
        None,
        true,
        || {
            let shop_ids = shop_ids.clone();
            let api_key = api_key.clone();
            let org = org_id.clone();
            async move { subscribe_to_app_logs(client, &shop_ids, &api_key, &org).await }
        },
        |logs| {
            let store_name = store_name.clone();
            let logs_dir = logs_dir.clone();
            let owned: Vec<_> = logs.to_vec();
            async move {
                for log in &owned {
                    let line = format_log_text(log, &store_name);
                    print!("{line}");
                    if let Some(ref dir) = logs_dir {
                        let _ = write_app_logs_to_file(log, &store_name, dir);
                    }
                }
                Ok(())
            }
        },
    );

    tokio::select! {
        _ = abort.cancelled() => Ok(()),
        res = loop_fut => res,
    }
}
