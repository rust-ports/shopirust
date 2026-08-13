//! GraphiQL explorer served from CDN for `app dev`.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct GraphiqlOptions {
    pub port: u16,
    pub app_name: String,
    pub app_url: String,
    pub store_fqdn: String,
    pub key: String,
    pub api_key: String,
    pub api_secret: String,
    pub graphql_url: Option<String>,
}

#[derive(Clone)]
struct GraphiqlState {
    key: String,
    app_name: String,
    app_url: String,
    store: String,
    graphql_url: String,
}

pub fn setup_graphiql_server_process(opts: GraphiqlOptions) -> DevProcess {
    DevProcess::new("graphiql", DevProcessKind::Graphiql, move |ctx| {
        run_graphiql(ctx.abort, opts)
    })
}

async fn run_graphiql(abort: CancellationToken, opts: GraphiqlOptions) -> Result<(), AppError> {
    let graphql_url = opts.graphql_url.clone().unwrap_or_else(|| {
        format!("https://{}/admin/api/unstable/graphql.json", opts.store_fqdn)
    });
    let state = GraphiqlState {
        key: opts.key.clone(),
        app_name: opts.app_name.clone(),
        app_url: opts.app_url.clone(),
        store: opts.store_fqdn.clone(),
        graphql_url,
    };

    let app = Router::new()
        .route("/graphiql", get(graphiql_page))
        .route("/graphql", post(graphql_proxy))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], opts.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::message(format!("failed to bind GraphiQL on {addr}: {e}")))?;
    tracing::info!(
        target: "app_dev",
        "GraphiQL listening on http://localhost:{}/graphiql",
        opts.port
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(abort.cancelled_owned())
        .await
        .map_err(|e| AppError::message(format!("GraphiQL server error: {e}")))
}

#[derive(Deserialize)]
struct KeyQuery {
    key: Option<String>,
}

async fn graphiql_page(
    State(state): State<GraphiqlState>,
    Query(q): Query<KeyQuery>,
) -> impl IntoResponse {
    if q.key.as_deref() != Some(state.key.as_str()) {
        return Html(format!(
            "<html><body><h1>Unauthorized</h1><p>Missing or invalid GraphiQL key.</p></body></html>"
        ));
    }
    Html(graphiql_cdn_html(
        &state.app_name,
        &state.app_url,
        &state.store,
        &state.key,
    ))
}

async fn graphql_proxy(
    State(state): State<GraphiqlState>,
    Query(q): Query<KeyQuery>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if q.key.as_deref() != Some(state.key.as_str()) && !q.key.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"errors": [{"message": "invalid key"}]})),
        );
    }
    let client = reqwest::Client::new();
    match client.post(&state.graphql_url).json(&body).send().await {
        Ok(resp) => {
            let status = resp.status();
            let json: Value = resp.json().await.unwrap_or(Value::Null);
            (
                axum::http::StatusCode::from_u16(status.as_u16())
                    .unwrap_or(axum::http::StatusCode::OK),
                Json(json),
            )
        }
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"errors": [{"message": e.to_string()}]})),
        ),
    }
}

fn graphiql_cdn_html(app_name: &str, app_url: &str, store: &str, key: &str) -> String {
    let escaped_name = app_name.replace('<', "&lt;");
    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>GraphiQL — {escaped_name}</title>
  <link rel="stylesheet" href="https://unpkg.com/graphiql@3.8.3/graphiql.min.css" />
  <style>html,body,#graphiql {{ height: 100%; margin: 0; }}</style>
</head>
<body>
  <div id="graphiql">Loading GraphiQL…</div>
  <script crossorigin src="https://unpkg.com/react@18/umd/react.production.min.js"></script>
  <script crossorigin src="https://unpkg.com/react-dom@18/umd/react-dom.production.min.js"></script>
  <script src="https://unpkg.com/graphiql@3.8.3/graphiql.min.js"></script>
  <script>
    const KEY = {key_json};
    const fetcher = GraphiQL.createFetcher({{
      url: '/graphql?key=' + encodeURIComponent(KEY),
    }});
    ReactDOM.createRoot(document.getElementById('graphiql')).render(
      React.createElement(GraphiQL, {{ fetcher }})
    );
  </script>
  <footer style="position:fixed;bottom:0;left:0;padding:4px 8px;font:12px system-ui;background:#fff;opacity:.85">
    App: <a href="{app_url}">{escaped_name}</a> · Store: {store}
  </footer>
</body>
</html>"#,
        key_json = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into()),
        app_url = app_url,
        store = store,
        escaped_name = escaped_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_includes_cdn_graphiql() {
        let html = graphiql_cdn_html("Demo", "https://app", "shop.myshopify.com", "k");
        assert!(html.contains("unpkg.com/graphiql"));
        assert!(html.contains("GraphiQL"));
        assert!(html.contains("shop.myshopify.com"));
    }
}
