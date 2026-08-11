//! HTTP server for UI extension preview.

pub mod middlewares;
pub mod utilities;

use crate::models::extensions::ExtensionInstance;
use crate::services::dev::extension::payload::store::ExtensionsPayloadStore;
use crate::services::dev::extension::websocket::handle_ws_socket;
use crate::services::dev::extension::ExtensionDevOptions;
use crate::error::AppError;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use middlewares::{content_type_for_path, resolve_asset_file, EXTENSION_JSON_CONTENT_TYPE};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use utilities::{get_extension_point_redirect_url, get_redirect_url};

#[derive(Clone)]
pub struct ServerState {
    pub options: ExtensionDevOptions,
    pub payload_store: Arc<Mutex<ExtensionsPayloadStore>>,
    pub extensions: Arc<Mutex<Vec<ExtensionInstance>>>,
    pub bundle_path: PathBuf,
    pub update_tx: broadcast::Sender<Vec<String>>,
    pub manifest_version: String,
}

/// Bind localhost:{port} and serve until cancelled.
pub async fn serve_extension_server(
    port: u16,
    state: ServerState,
    cancel: CancellationToken,
) -> Result<(), AppError> {
    let app = build_extension_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::message(format!("failed to bind extension server on {addr}: {e}")))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(cancel.cancelled_owned())
        .await
        .map_err(|e| AppError::message(format!("extension server error: {e}")))
}

pub fn build_extension_router(state: ServerState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(redirect_root))
        .route("/extensions/dev-console", get(dev_console_stub))
        .route(
            "/extensions/:extension_id/assets/*asset_path",
            get(extension_asset),
        )
        .route(
            "/extensions/:extension_id/:extension_point_target",
            get(extension_point_redirect),
        )
        .route("/extensions/:extension_id", get(extension_payload))
        .route("/extensions", get(ws_or_payload))
        .with_state(state)
        .layer(cors)
}

async fn ws_or_payload(
    ws: Option<WebSocketUpgrade>,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    let is_ws = headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if is_ws {
        if let Some(ws) = ws {
            return ws
                .on_upgrade(move |socket| {
                    let store = state.payload_store.clone();
                    let tx = state.update_tx.clone();
                    let version = state.manifest_version.clone();
                    async move {
                        handle_ws_socket(socket, store, tx, version).await;
                    }
                })
                .into_response();
        }
    }
    extensions_payload(State(state)).await.into_response()
}

async fn redirect_root() -> Redirect {
    Redirect::temporary("/extensions/dev-console")
}

async fn dev_console_stub() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>Shopify Dev Console</title>
  <style>
    :root { color-scheme: light dark; font-family: ui-sans-serif, system-ui, sans-serif; }
    body { margin: 0; padding: 2rem; line-height: 1.5; }
    h1 { font-size: 1.5rem; margin: 0 0 0.5rem; }
    .muted { opacity: 0.7; }
    #status { margin-top: 1rem; padding: 0.75rem 1rem; border-radius: 8px; background: #1111; border: 1px solid #8884; }
    #log { margin-top: 1rem; font-family: ui-monospace, monospace; font-size: 12px; white-space: pre-wrap; max-height: 40vh; overflow: auto; }
    code { background: #8882; padding: 0.1em 0.35em; border-radius: 4px; }
  </style>
</head>
<body>
  <h1>Dev console</h1>
  <p class="muted">Minimal Rust-port console. Connects to the extension WebSocket at <code>/extensions</code>.</p>
  <div id="status">Connecting…</div>
  <div id="log"></div>
  <script>
    const statusEl = document.getElementById('status');
    const logEl = document.getElementById('log');
    function log(msg) {
      logEl.textContent += msg + '\n';
      logEl.scrollTop = logEl.scrollHeight;
    }
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = proto + '//' + location.host + '/extensions';
    const ws = new WebSocket(wsUrl);
    ws.addEventListener('open', () => {
      statusEl.textContent = 'WebSocket connected: ' + wsUrl;
      try {
        ws.send(JSON.stringify({ event: 'connected', data: { client: 'rust-dev-console' } }));
      } catch (e) { log('send error: ' + e); }
    });
    ws.addEventListener('message', (ev) => {
      log('← ' + ev.data);
      try {
        const msg = JSON.parse(ev.data);
        if (msg && msg.event) statusEl.textContent = 'Last event: ' + msg.event;
      } catch (_) {}
    });
    ws.addEventListener('close', () => { statusEl.textContent = 'WebSocket closed'; });
    ws.addEventListener('error', () => { statusEl.textContent = 'WebSocket error (is the extension server running?)'; });
  </script>
</body>
</html>"#,
    )
}

async fn extensions_payload(State(state): State<ServerState>) -> impl IntoResponse {
    let store = state.payload_store.lock().unwrap();
    let body = serde_json::to_value(store.get_raw_payload()).unwrap_or(serde_json::json!({}));
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, EXTENSION_JSON_CONTENT_TYPE)],
        Json(body),
    )
}

async fn extension_payload(
    Path(extension_id): Path<String>,
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    let extensions = state.extensions.lock().unwrap().clone();
    let Some(extension) = extensions
        .iter()
        .find(|e| e.dev_uuid.as_deref() == Some(&extension_id))
    else {
        return (
            StatusCode::NOT_FOUND,
            format!("Extension with id {extension_id} not found"),
        )
            .into_response();
    };

    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if accept.starts_with("text/html") {
        if extension.type_name() == "checkout_post_purchase" {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                "<html><body>post_purchase stub</body></html>",
            )
                .into_response();
        }
        let opts = state.options.to_store_options(state.options.websocket_url());
        let url = get_redirect_url(extension, &opts);
        return Redirect::temporary(&url).into_response();
    }

    let store_opts = state.options.to_store_options(state.options.websocket_url());
    let payload = match crate::services::dev::extension::payload::get_ui_extension_payload(
        extension,
        &state.bundle_path,
        &store_opts,
        None,
        None,
        None,
    ) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let body = serde_json::json!({
        "app": { "apiKey": state.options.api_key },
        "version": state.options.manifest_version,
        "root": { "url": format!("{}/extensions", state.options.url.trim_end_matches('/')) },
        "socket": { "url": state.options.websocket_url() },
        "devConsole": { "url": format!("{}/extensions/dev-console", state.options.url.trim_end_matches('/')) },
        "store": state.options.store_fqdn,
        "extension": payload,
    });
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, EXTENSION_JSON_CONTENT_TYPE)],
        Json(body),
    )
        .into_response()
}

async fn extension_point_redirect(
    Path((extension_id, extension_point_target)): Path<(String, String)>,
    State(state): State<ServerState>,
) -> Response {
    let extensions = state.extensions.lock().unwrap().clone();
    let Some(extension) = extensions
        .iter()
        .find(|e| e.dev_uuid.as_deref() == Some(&extension_id))
    else {
        return (
            StatusCode::NOT_FOUND,
            format!("Extension with id {extension_id} not found"),
        )
            .into_response();
    };

    if extension.type_name() != "checkout_post_purchase"
        && !extension.has_extension_point_target(&extension_point_target)
    {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "Extension with id {extension_id} has not configured the \"{extension_point_target}\" extension target"
            ),
        )
            .into_response();
    }

    let opts = state.options.to_store_options(state.options.websocket_url());
    match get_extension_point_redirect_url(&extension_point_target, extension, &opts) {
        Some(url) => Redirect::temporary(&url).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            format!(
                "Redirect url can't be constructed for extension with id {extension_id} and extension target \"{extension_point_target}\""
            ),
        )
            .into_response(),
    }
}

async fn extension_asset(
    Path((extension_id, asset_path)): Path<(String, String)>,
    State(state): State<ServerState>,
) -> Response {
    let extensions = state.extensions.lock().unwrap().clone();
    let Some(extension) = extensions
        .iter()
        .find(|e| e.dev_uuid.as_deref() == Some(&extension_id))
    else {
        return (
            StatusCode::NOT_FOUND,
            format!("Extension with id {extension_id} not found"),
        )
            .into_response();
    };

    let store = state.payload_store.lock().unwrap();
    let resolver = store.get_asset_resolver(&extension_id);
    let filesystem_path = resolver
        .and_then(|r| r.get(&asset_path))
        .cloned()
        .unwrap_or_else(|| asset_path.clone());

    let output_path = extension.get_output_path_for_directory(&state.bundle_path);
    match resolve_asset_file(&output_path, &filesystem_path) {
        Ok(path) => match std::fs::read(&path) {
            Ok(bytes) => {
                let ct = content_type_for_path(&path);
                ([(header::CONTENT_TYPE, ct)], bytes).into_response()
            }
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        },
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::services::dev::extension::payload::store::get_extensions_payload_store_raw_payload;
    use axum::body::Body;
    use axum::http::Request;
    use std::collections::HashMap;
    use tempfile::tempdir;
    use tower::ServiceExt;

    fn test_state(dir: &std::path::Path) -> ServerState {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut ext = ExtensionInstance::new(
            "my-ext",
            dir.join("ext"),
            dir.join("ext/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        ext.dev_uuid = Some("dev-1".into());
        std::fs::create_dir_all(dir.join("bundle/my-ext")).unwrap();
        std::fs::write(dir.join("bundle/my-ext/my-ext.js"), b"ok").unwrap();
        ext.output_path = Some(PathBuf::from("my-ext/my-ext.js"));

        let options = ExtensionDevOptions {
            extensions: vec![ext.clone()],
            id: Some("app".into()),
            app_name: "App".into(),
            app_directory: dir.to_path_buf(),
            api_key: "key".into(),
            url: "http://localhost:9293".into(),
            port: 9293,
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "1".into(),
            granted_scopes: vec![],
            checkout_cart_url: Some("/cart/1:1".into()),
            subscription_product_url: None,
            manifest_version: "3".into(),
            build_directory: Some(dir.join("bundle")),
        };
        let store_opts = options.to_store_options(options.websocket_url());
        let mut resolvers = HashMap::new();
        let raw = get_extensions_payload_store_raw_payload(
            &store_opts,
            &[ext.clone()],
            &dir.join("bundle"),
            &mut resolvers,
        )
        .unwrap();
        let (tx, _) = broadcast::channel(16);
        ServerState {
            options,
            payload_store: Arc::new(Mutex::new(ExtensionsPayloadStore::new(
                raw, store_opts, resolvers,
            ))),
            extensions: Arc::new(Mutex::new(vec![ext])),
            bundle_path: dir.join("bundle"),
            update_tx: tx,
            manifest_version: "3".into(),
        }
    }

    #[tokio::test]
    async fn root_redirects_to_dev_console() {
        let dir = tempdir().unwrap();
        let app = build_extension_router(test_state(dir.path()));
        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
    }

    #[tokio::test]
    async fn extensions_json_payload() {
        let dir = tempdir().unwrap();
        let app = build_extension_router(test_state(dir.path()));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/extensions")
                    .header("accept", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn asset_served_with_path_jail() {
        let dir = tempdir().unwrap();
        let app = build_extension_router(test_state(dir.path()));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/extensions/dev-1/assets/my-ext.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let app = build_extension_router(test_state(dir.path()));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/extensions/dev-1/assets/../../etc/passwd")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_extension_404() {
        let dir = tempdir().unwrap();
        let app = build_extension_router(test_state(dir.path()));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/extensions/missing")
                    .header("accept", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
