//! App file watcher process — wraps [`AppEventWatcher`].

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::services::dev::app_events::AppEventWatcher;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub fn setup_app_watcher_process(app_watcher: Arc<AppEventWatcher>) -> DevProcess {
    DevProcess::new("app-preview", DevProcessKind::AppWatcher, move |ctx| {
        run_app_watcher(ctx.abort, app_watcher)
    })
}

async fn run_app_watcher(
    abort: CancellationToken,
    app_watcher: Arc<AppEventWatcher>,
) -> Result<(), AppError> {
    app_watcher.start(abort, true).await
}
