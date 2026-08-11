//! Minimal GraphiQL HTTP stub for `app dev`.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct GraphiqlOptions {
    pub port: u16,
    pub app_name: String,
    pub app_url: String,
    pub store_fqdn: String,
    pub key: String,
}

pub fn setup_graphiql_server_process(opts: GraphiqlOptions) -> DevProcess {
    DevProcess::new("graphiql", DevProcessKind::Graphiql, move |ctx| {
        run_graphiql(ctx.abort, opts)
    })
}

async fn run_graphiql(abort: CancellationToken, opts: GraphiqlOptions) -> Result<(), AppError> {
    let key = opts.key.clone();
    let app_name = opts.app_name.clone();
    let app_url = opts.app_url.clone();
    let store = opts.store_fqdn.clone();

    let app = Router::new().route(
        "/graphiql",
        get(move || {
            let key = key.clone();
            let app_name = app_name.clone();
            let app_url = app_url.clone();
            let store = store.clone();
            async move {
                Html(format!(
                    r#"<!doctype html>
<html><head><meta charset="utf-8"><title>GraphiQL — {app_name}</title></head>
<body style="font-family:system-ui;padding:2rem">
  <h1>GraphiQL (stub)</h1>
  <p>App: <a href="{app_url}">{app_name}</a></p>
  <p>Store: {store}</p>
  <p>Key accepted: <code>{key}</code></p>
  <p>Full GraphiQL explorer is not packaged in the Rust port yet.</p>
</body></html>"#
                ))
            }
        }),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], opts.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::message(format!("failed to bind GraphiQL on {addr}: {e}")))?;
    tracing::info!(target: "app_dev", "GraphiQL stub listening on http://localhost:{}/graphiql", opts.port);

    axum::serve(listener, app)
        .with_graceful_shutdown(abort.cancelled_owned())
        .await
        .map_err(|e| AppError::message(format!("GraphiQL server error: {e}")))
}
