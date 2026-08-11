//! App logs polling during `app dev` — reuses [`AppLogsPoller`].

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::services::app_logs::{
    format_log_text, subscribe_to_app_logs, AppLogsPoller, PollBackend, PollFilters,
};
use cli_api::DeveloperPlatformClient;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppLogsPollingOptions {
    pub organization_id: String,
    pub api_key: String,
    pub shop_ids: Vec<i64>,
    pub store_name: String,
}

/// Build a process that idles until cancel (client is not `'static`).
/// Prefer [`run_app_logs_polling`] from the orchestrator when a live client is available.
pub fn setup_app_logs_polling_process(opts: AppLogsPollingOptions) -> DevProcess {
    DevProcess::new(
        "app-logs",
        DevProcessKind::AppLogsPolling,
        move |ctx| async move {
            let _ = opts;
            ctx.abort.cancelled().await;
            Ok(())
        },
    )
}

/// Run logs polling with a live developer-platform client until cancelled.
pub async fn run_app_logs_polling(
    abort: CancellationToken,
    client: &dyn DeveloperPlatformClient,
    opts: AppLogsPollingOptions,
) -> Result<(), AppError> {
    let jwt = subscribe_to_app_logs(
        client,
        &opts.shop_ids,
        &opts.api_key,
        &opts.organization_id,
    )
    .await?;

    let mut poller = AppLogsPoller::new(jwt, PollFilters::default());
    let backend = PollBackend::Platform {
        client,
        organization_id: opts.organization_id.clone(),
    };
    let store_name = opts.store_name.clone();
    let shop_ids = opts.shop_ids.clone();
    let api_key = opts.api_key.clone();
    let org_id = opts.organization_id.clone();

    let loop_fut = poller.run_loop(
        &backend,
        None,
        true,
        || {
            let client = client;
            let shop_ids = shop_ids.clone();
            let api_key = api_key.clone();
            let org = org_id.clone();
            async move { subscribe_to_app_logs(client, &shop_ids, &api_key, &org).await }
        },
        |logs| {
            let store_name = store_name.clone();
            let owned: Vec<_> = logs.to_vec();
            async move {
                for log in &owned {
                    let line = format_log_text(log, &store_name);
                    print!("{line}");
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
