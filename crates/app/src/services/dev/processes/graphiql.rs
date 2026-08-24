//! GraphiQL explorer served from CDN for `app dev`.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

type HmacSha256 = Hmac<Sha256>;

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
    /// Override OAuth token endpoint (tests). Default: `https://{store}/admin/oauth/access_token`.
    pub token_url: Option<String>,
}

#[derive(Clone)]
struct GraphiqlState {
    key: String,
    app_name: String,
    app_url: String,
    store: String,
    graphql_url: String,
    api_key: String,
    api_secret: String,
    token_url: String,
    token: Arc<Mutex<Option<String>>>,
}

/// HMAC-SHA256 hex of `graphiql:{store_fqdn}` keyed by the app secret (upstream `deriveGraphiQLKey`).
pub fn derive_graphiql_key(api_secret: &str, store_fqdn: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes()).expect("HMAC accepts any key");
    mac.update(format!("graphiql:{store_fqdn}").as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn resolve_graphiql_key(provided: Option<&str>, api_secret: &str, store_fqdn: &str) -> String {
    provided
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| derive_graphiql_key(api_secret, store_fqdn))
}

pub fn setup_graphiql_server_process(opts: GraphiqlOptions) -> DevProcess {
    DevProcess::new("graphiql", DevProcessKind::Graphiql, move |ctx| {
        run_graphiql(ctx.abort, opts)
    })
}

fn graphiql_router(state: GraphiqlState) -> Router {
    Router::new()
        .route("/graphiql", get(graphiql_page))
        .route("/graphiql/status", get(graphiql_status))
        .route("/graphiql/ping", get(|| async { "pong" }))
        .route("/graphql", post(graphql_proxy))
        .route("/graphiql/graphql.json", post(graphql_proxy))
        .with_state(state)
}

async fn run_graphiql(abort: CancellationToken, opts: GraphiqlOptions) -> Result<(), AppError> {
    let graphql_url = opts.graphql_url.clone().unwrap_or_else(|| {
        format!(
            "https://{}/admin/api/unstable/graphql.json",
            opts.store_fqdn
        )
    });
    let token_url = opts
        .token_url
        .clone()
        .unwrap_or_else(|| format!("https://{}/admin/oauth/access_token", opts.store_fqdn));
    let state = GraphiqlState {
        key: opts.key.clone(),
        app_name: opts.app_name.clone(),
        app_url: opts.app_url.clone(),
        store: opts.store_fqdn.clone(),
        graphql_url,
        api_key: opts.api_key.clone(),
        api_secret: opts.api_secret.clone(),
        token_url,
        token: Arc::new(Mutex::new(None)),
    };

    let app = graphiql_router(state);
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

fn unauthorized_html() -> Html<String> {
    Html(
        "<html><body><h1>Unauthorized</h1><p>Missing or invalid GraphiQL key. Check that your app is installed, and try again.</p></body></html>"
            .to_string(),
    )
}

async fn graphiql_page(
    State(state): State<GraphiqlState>,
    Query(q): Query<KeyQuery>,
) -> impl IntoResponse {
    if q.key.as_deref() != Some(state.key.as_str()) {
        return unauthorized_html();
    }
    Html(graphiql_cdn_html(
        &state.app_name,
        &state.app_url,
        &state.store,
        &state.key,
    ))
}

async fn graphiql_status(State(state): State<GraphiqlState>) -> impl IntoResponse {
    match current_token(&state).await {
        Ok(_) => Json(serde_json::json!({
            "status": "OK",
            "storeFqdn": state.store,
            "appName": state.app_name,
            "appUrl": state.app_url,
        })),
        Err(_) => Json(serde_json::json!({ "status": "UNAUTHENTICATED" })),
    }
}

async fn graphql_proxy(
    State(state): State<GraphiqlState>,
    Query(q): Query<KeyQuery>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if q.key.as_deref() != Some(state.key.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"errors": [{"message": "invalid key"}]})),
        );
    }
    let Ok(token) = current_token(&state).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                serde_json::json!({"errors": [{"message": "Failed to refresh credentials. Check that your app is installed, and try again."}]}),
            ),
        );
    };
    match proxy_once(&state, &body, &token).await {
        Ok((status, _json)) if status == StatusCode::UNAUTHORIZED => {
            match refresh_token(&state).await {
                Ok(fresh) => match proxy_once(&state, &body, &fresh).await {
                    Ok((st, j)) => (st, Json(j)),
                    Err(e) => (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({"errors": [{"message": e}]})),
                    ),
                },
                Err(_) => (
                    StatusCode::UNAUTHORIZED,
                    Json(
                        serde_json::json!({"errors": [{"message": "Failed to refresh credentials. Check that your app is installed, and try again."}]}),
                    ),
                ),
            }
        }
        Ok((status, json)) => (status, Json(json)),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"errors": [{"message": e}]})),
        ),
    }
}

async fn proxy_once(
    state: &GraphiqlState,
    body: &Value,
    token: &str,
) -> Result<(StatusCode, Value), String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.graphql_url)
        .header("X-Shopify-Access-Token", token)
        .header("Accept", "application/json")
        .header("User-Agent", "ShopifyCLIGraphiQL/0.1.0")
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::OK);
    let json: Value = resp.json().await.unwrap_or(Value::Null);
    Ok((status, json))
}

async fn current_token(state: &GraphiqlState) -> Result<String, String> {
    if let Some(cached) = state.token.lock().unwrap().clone() {
        return Ok(cached);
    }
    mint_token(state).await
}

async fn refresh_token(state: &GraphiqlState) -> Result<String, String> {
    *state.token.lock().unwrap() = None;
    mint_token(state).await
}

async fn mint_token(state: &GraphiqlState) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.token_url)
        .json(&serde_json::json!({
            "client_id": state.api_key,
            "client_secret": state.api_secret,
            "grant_type": "client_credentials",
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "Token request failed with status {}",
            resp.status()
        ));
    }
    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    let token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token request returned no access_token".to_string())?;
    *state.token.lock().unwrap() = Some(token.to_string());
    Ok(token.to_string())
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
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_state(graphql: &str, token: &str, key: &str) -> GraphiqlState {
        GraphiqlState {
            key: key.into(),
            app_name: "Demo".into(),
            app_url: "https://app".into(),
            store: "shop.myshopify.com".into(),
            graphql_url: graphql.into(),
            api_key: "key".into(),
            api_secret: "secret".into(),
            token_url: token.into(),
            token: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn html_includes_cdn_graphiql() {
        let html = graphiql_cdn_html("Demo", "https://app", "shop.myshopify.com", "k");
        assert!(html.contains("unpkg.com/graphiql"));
        assert!(html.contains("GraphiQL"));
        assert!(html.contains("shop.myshopify.com"));
    }

    #[test]
    fn derive_graphiql_key_is_stable_hex() {
        let a = derive_graphiql_key("secret", "shop.myshopify.com");
        let b = derive_graphiql_key("secret", "shop.myshopify.com");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert_ne!(derive_graphiql_key("secret", "other.myshopify.com"), a);
        assert_eq!(
            resolve_graphiql_key(Some(" explicit "), "s", "shop.myshopify.com"),
            "explicit"
        );
        assert_eq!(
            resolve_graphiql_key(None, "secret", "shop.myshopify.com"),
            a
        );
    }

    #[tokio::test]
    async fn proxy_sends_access_token_header() {
        let token_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/admin/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "shpat_live"
            })))
            .mount(&token_server)
            .await;

        let gql_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/admin/api/unstable/graphql.json"))
            .and(header("X-Shopify-Access-Token", "shpat_live"))
            .and(body_partial_json(
                serde_json::json!({"query": "{ shop { name } }"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "shop": { "name": "Demo" } }
            })))
            .mount(&gql_server)
            .await;

        let state = test_state(
            &format!("{}/admin/api/unstable/graphql.json", gql_server.uri()),
            &format!("{}/admin/oauth/access_token", token_server.uri()),
            "k",
        );
        let app = graphiql_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql?key=k")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"{ shop { name } }"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["data"]["shop"]["name"], "Demo");
    }

    #[tokio::test]
    async fn proxy_rejects_wrong_key() {
        let state = test_state("http://example/graphql", "http://example/token", "k");
        let app = graphiql_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql?key=wrong")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"{ shop { name } }"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn status_unauthenticated_without_token() {
        let token_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({})))
            .mount(&token_server)
            .await;
        let state = test_state(
            "http://example/graphql",
            &format!("{}/admin/oauth/access_token", token_server.uri()),
            "k",
        );
        let app = graphiql_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/graphiql/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "UNAUTHENTICATED");
    }
}
