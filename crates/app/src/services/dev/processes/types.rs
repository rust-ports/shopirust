//! Dev process types (prefix + async runner).

use crate::error::AppError;
use std::future::Future;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

/// Context passed to each concurrent dev process.
#[derive(Clone)]
pub struct DevProcessContext {
    pub abort: CancellationToken,
    pub prefix: String,
}

/// Async process body.
pub type DevProcessFn = Box<
    dyn FnOnce(DevProcessContext) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send>>
        + Send,
>;

/// A named concurrent process launched by `app dev`.
pub struct DevProcess {
    pub prefix: String,
    pub kind: DevProcessKind,
    pub run: DevProcessFn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevProcessKind {
    Web,
    PreviewableExtension,
    DraftableExtension,
    ThemeAppExtension,
    UninstallWebhook,
    Graphiql,
    DevSession,
    AppLogsPolling,
    AppWatcher,
    ProxyServer,
}

impl DevProcess {
    pub fn new<F, Fut>(prefix: impl Into<String>, kind: DevProcessKind, f: F) -> Self
    where
        F: FnOnce(DevProcessContext) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        Self {
            prefix: prefix.into(),
            kind,
            run: Box::new(move |ctx| Box::pin(f(ctx))),
        }
    }
}
