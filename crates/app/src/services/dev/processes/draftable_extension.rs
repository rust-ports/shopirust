//! Draftable extension process (Partners path) — stub until Partners push lands.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct DraftableExtensionOptions {
    pub api_key: String,
    pub proxy_url: String,
}

/// Partners apps without Dev Sessions use draftable extension push. Stub for now.
pub fn setup_draftable_extensions_process(opts: DraftableExtensionOptions) -> DevProcess {
    DevProcess::new(
        "extensions",
        DevProcessKind::DraftableExtension,
        move |ctx| run_draftable(ctx.abort, opts),
    )
}

async fn run_draftable(
    abort: CancellationToken,
    opts: DraftableExtensionOptions,
) -> Result<(), AppError> {
    tracing::info!(
        target: "app_dev",
        "draftable extension push stub (api_key={}, proxy={})",
        opts.api_key,
        opts.proxy_url
    );
    abort.cancelled().await;
    Ok(())
}
