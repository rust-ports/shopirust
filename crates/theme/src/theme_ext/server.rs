use crate::dev::{
    allowed_hosts, can_proxy_request, inject_cdn_proxy, inject_hot_reload_script,
    serialize_cookies, DevServerSession, HotReloadEvent, HotReloadPayload, LiveReloadMode,
};
use crate::filesystem::ThemeAsset;
use crate::theme_ext::fs::{
    get_extension_in_memory_templates, mount_theme_extension_file_system,
    replace_extension_templates_params, ThemeExtFsEventName, ThemeExtFsEventPayload,
    ThemeExtensionFileSystem,
};
use crate::theme_ext::session::empty_dev_session;
use crate::utilities::host_theme_manager::storefront_origin;
use axum::extract::{Path as AxumPath, Request, State};
use axum::http::header::{CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use futures::Stream;
use percent_encoding::percent_decode_str;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub const DEFAULT_THEME_EXT_HOST: &str = "127.0.0.1";
pub const DEFAULT_THEME_EXT_PORT: u16 = 9293;

const HOT_RELOAD_VERSION: &str = "1";

#[derive(Debug, thiserror::Error)]
pub enum ThemeExtServerError {
    #[error("Unable to bind theme extension server to {0}: {1}")]
    Bind(SocketAddr, std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ThemeExtServerContext {
    pub host: String,
    pub port: u16,
    pub directory: PathBuf,
    pub theme_id: i64,
    pub filesystem: ThemeExtensionFileSystem,
    pub session: DevServerSession,
    /// Override store origin (`http://127.0.0.1:port`) for tests.
    pub store_origin: Option<String>,
}

#[derive(Clone)]
struct AppState {
    ctx: Arc<ThemeExtServerContext>,
    reload_tx: broadcast::Sender<HotReloadEvent>,
    client: reqwest::Client,
}

pub struct ThemeExtServerHandle {
    shutdown_tx: watch::Sender<bool>,
    join: tokio::task::JoinHandle<()>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl ThemeExtServerHandle {
    pub async fn close(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join.await;
    }
}

/// Builds the extension-server context: mounts FS, forces hot-reload semantics.
pub fn build_theme_extension_context(
    directory: impl Into<PathBuf>,
    theme_id: i64,
    port: Option<u16>,
) -> ThemeExtServerContext {
    let directory = directory.into();
    let filesystem = mount_theme_extension_file_system(&directory);
    filesystem.ready();
    ThemeExtServerContext {
        host: DEFAULT_THEME_EXT_HOST.into(),
        port: port.unwrap_or(DEFAULT_THEME_EXT_PORT),
        directory,
        theme_id,
        filesystem,
        session: empty_dev_session(""),
        store_origin: None,
    }
}

/// Starts a minimal Axum theme-extension server on `127.0.0.1:9293` by default.
///
/// Reuses host-validation and hot-reload patterns from `dev.rs`. Live reload is
/// unconditional (upstream always sets `liveReload: 'hot-reload'`).
pub async fn run_theme_extension_server(
    ctx: ThemeExtServerContext,
) -> Result<ThemeExtServerHandle, ThemeExtServerError> {
    let addr = SocketAddr::new(
        ctx.host
            .parse()
            .unwrap_or_else(|_| DEFAULT_THEME_EXT_HOST.parse().expect("default host")),
        ctx.port,
    );
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| ThemeExtServerError::Bind(addr, error))?;

    let (reload_tx, _) = broadcast::channel(256);
    attach_hot_reload_listeners(&ctx.filesystem, ctx.theme_id, reload_tx.clone());
    let watcher = ctx.filesystem.start_watcher().ok();

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = AppState {
        ctx: Arc::new(ctx),
        reload_tx,
        client: proxy_client(),
    };
    let app = build_theme_extension_router(state);

    let join = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        });
        if let Err(error) = server.await {
            eprintln!("Theme extension server error: {error}");
        }
    });

    Ok(ThemeExtServerHandle {
        shutdown_tx,
        join,
        _watcher: watcher,
    })
}

fn proxy_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn build_theme_extension_router(state: AppState) -> Router {
    Router::new()
        .route("/__theme_dev/hot-reload", get(hot_reload))
        .route("/assets/*path", get(asset))
        .fallback(any(fallback))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            validate_host_request,
        ))
        .with_state(state)
}

async fn validate_host_request(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !valid_host(&state.ctx, request.headers()) {
        return (StatusCode::BAD_REQUEST, "Invalid Host header").into_response();
    }
    next.run(request).await
}

/// Builds a router from a server context (for tests / embedding).
pub fn theme_extension_router_from_context(ctx: ThemeExtServerContext) -> Router {
    theme_extension_app(ctx).0
}

fn theme_extension_app(ctx: ThemeExtServerContext) -> (Router, broadcast::Sender<HotReloadEvent>) {
    let (reload_tx, _) = broadcast::channel(256);
    attach_hot_reload_listeners(&ctx.filesystem, ctx.theme_id, reload_tx.clone());
    let state = AppState {
        ctx: Arc::new(ctx),
        reload_tx: reload_tx.clone(),
        client: proxy_client(),
    };
    (build_theme_extension_router(state), reload_tx)
}

fn attach_hot_reload_listeners(
    filesystem: &ThemeExtensionFileSystem,
    theme_id: i64,
    reload_tx: broadcast::Sender<HotReloadEvent>,
) {
    let on_update = {
        let reload_tx = reload_tx.clone();
        Arc::new(move |payload: ThemeExtFsEventPayload| {
            trigger_hot_reload(
                &reload_tx,
                theme_id,
                &payload.file_key,
                payload.content.as_deref(),
            );
        })
    };
    let add = Arc::clone(&on_update);
    let change = Arc::clone(&on_update);
    filesystem.add_event_listener(ThemeExtFsEventName::Add, move |payload| add(payload));
    filesystem.add_event_listener(ThemeExtFsEventName::Change, move |payload| change(payload));
    let reload_tx_delete = reload_tx;
    filesystem.add_event_listener(ThemeExtFsEventName::Unlink, move |payload| {
        let _ = reload_tx_delete.send(HotReloadEvent::Delete {
            version: HOT_RELOAD_VERSION.into(),
            sync: "local".into(),
            theme_id: theme_id.to_string(),
            key: payload.file_key,
        });
    });
}

pub fn trigger_hot_reload(
    reload_tx: &broadcast::Sender<HotReloadEvent>,
    theme_id: i64,
    key: &str,
    content: Option<&str>,
) {
    let mut replace_templates = BTreeMap::new();
    if let Some(content) = content {
        replace_templates.insert(key.to_string(), content.to_string());
    }
    let _ = reload_tx.send(HotReloadEvent::Update {
        version: HOT_RELOAD_VERSION.into(),
        sync: "local".into(),
        theme_id: theme_id.to_string(),
        key: key.into(),
        payload: HotReloadPayload {
            replace_templates,
            is_theme_extension: true,
            ..Default::default()
        },
    });
}

async fn hot_reload(
    State(state): State<AppState>,
) -> Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>> {
    let rx = state.reload_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|event| {
        let event = match event {
            Ok(event) => event,
            Err(_) => return None,
        };
        let data = serde_json::to_string(&event).ok()?;
        Some(Ok(Event::default().data(data)))
    });
    let _ = state.reload_tx.send(HotReloadEvent::Open {
        version: HOT_RELOAD_VERSION.into(),
        pid: std::process::id().to_string(),
        theme_id: state.ctx.theme_id.to_string(),
    });
    let stream: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = Box::pin(stream);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn asset(State(state): State<AppState>, AxumPath(path): AxumPath<String>) -> Response {
    let key = format!("assets/{}", decode_path(&path));
    serve_extension_asset(&state.ctx.filesystem, &key)
}

async fn fallback(State(state): State<AppState>, request: Request) -> Response {
    if should_ignore(request.uri().path()) {
        return StatusCode::NO_CONTENT.into_response();
    }
    if can_proxy_request(request.method(), request.uri(), request.headers()) {
        return proxy_request(state, request).await;
    }
    render_storefront(state, request).await
}

async fn proxy_request(state: AppState, request: Request) -> Response {
    match forward_to_store(&state, request, false).await {
        Ok(response) => response,
        Err(_) => (StatusCode::BAD_GATEWAY, "Failed to reach storefront").into_response(),
    }
}

async fn render_storefront(state: AppState, request: Request) -> Response {
    match forward_to_store(&state, request, true).await {
        Ok(response) => response,
        Err(_) => (StatusCode::BAD_GATEWAY, "Failed to render storefront").into_response(),
    }
}

async fn forward_to_store(
    state: &AppState,
    request: Request,
    html: bool,
) -> Result<Response, reqwest::Error> {
    let Some(origin) = store_origin(&state.ctx) else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let mut query: BTreeMap<String, String> = request
        .uri()
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_default();
    if html {
        query
            .entry("preview_theme_id".into())
            .or_insert_with(|| state.ctx.theme_id.to_string());
        query.entry("_fd".into()).or_insert_with(|| "0".into());
        query.entry("pb".into()).or_insert_with(|| "0".into());
    }
    let mut url = format!("{origin}{path}");
    if !query.is_empty() {
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&query)
            .finish();
        url.push('?');
        url.push_str(&encoded);
    }

    let replace_extension_templates = if html {
        get_extension_in_memory_templates(&state.ctx.filesystem)
    } else {
        BTreeMap::new()
    };
    let (method, body) = if html && !replace_extension_templates.is_empty() {
        (
            Method::POST,
            Some(replace_extension_templates_params(
                &replace_extension_templates,
            )),
        )
    } else if method != Method::GET && method != Method::HEAD {
        let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        (method, Some(String::from_utf8_lossy(&bytes).into_owned()))
    } else {
        (method, None)
    };

    let mut builder = state.client.request(method, url);
    builder = builder.header("referer", format!("{origin}/"));
    let cookie = serialize_cookies(&state.ctx.session.session_cookies);
    if !cookie.is_empty() {
        builder = builder.header("cookie", cookie);
    }
    if let Some(body) = body {
        builder = builder
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body);
    }
    let response = builder.send().await?;
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("text/html; charset=utf-8")
        .to_string();
    let bytes = response.bytes().await.unwrap_or_default();
    if html || content_type.contains("text/html") {
        let mut body = String::from_utf8_lossy(&bytes).into_owned();
        let ext_files = state.ctx.filesystem.files();
        body = inject_cdn_proxy(
            &body,
            &state.ctx.session.store_fqdn,
            &BTreeMap::new(),
            &ext_files,
            false,
        );
        body = inject_hot_reload_script(&body, LiveReloadMode::HotReload);
        return Ok((status, [(CONTENT_TYPE, content_type)], body).into_response());
    }
    Ok((status, [(CONTENT_TYPE, content_type)], bytes).into_response())
}

fn store_origin(ctx: &ThemeExtServerContext) -> Option<String> {
    if let Some(origin) = &ctx.store_origin {
        let trimmed = origin.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let fqdn = ctx.session.store_fqdn.trim();
    if fqdn.is_empty() {
        return None;
    }
    Some(storefront_origin(fqdn))
}

fn serve_extension_asset(filesystem: &ThemeExtensionFileSystem, key: &str) -> Response {
    let Some(asset) = filesystem.files().get(key).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match asset_bytes(&asset) {
        Some(bytes) => {
            let mime = mime_guess::from_path(key)
                .first_or_octet_stream()
                .essence_str()
                .to_string();
            ([(CONTENT_TYPE, mime)], bytes).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn asset_bytes(asset: &ThemeAsset) -> Option<Vec<u8>> {
    if let Some(value) = &asset.value {
        return Some(value.as_bytes().to_vec());
    }
    asset
        .attachment
        .as_ref()
        .and_then(|attachment| BASE64_STANDARD.decode(attachment).ok())
}

pub fn valid_extension_host(ctx: &ThemeExtServerContext, headers: &HeaderMap) -> bool {
    valid_host(ctx, headers)
}

fn valid_host(ctx: &ThemeExtServerContext, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    allowed_hosts(&ctx.host, ctx.port).contains(&normalize_host_header(host))
}

fn normalize_host_header(host: &str) -> String {
    host.to_lowercase().replace(".:", ":")
}

fn should_ignore(path: &str) -> bool {
    [
        "/.well-known",
        "/shopify/monorail",
        "/mini-profiler-resources",
        "/web-pixels-manager",
        "/web-pixels@",
        "/wpm",
        "/services/",
        "/api/collect",
        "/cdn-cgi/challenge-platform",
    ]
    .iter()
    .any(|endpoint| path.starts_with(endpoint))
}

fn decode_path(path: &str) -> String {
    percent_decode_str(path).decode_utf8_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn context_at(port: u16) -> ThemeExtServerContext {
        ThemeExtServerContext {
            host: DEFAULT_THEME_EXT_HOST.into(),
            port,
            directory: PathBuf::from("tmp"),
            theme_id: 1,
            filesystem: mount_theme_extension_file_system("tmp"),
            session: empty_dev_session(""),
            store_origin: None,
        }
    }

    fn fixture_context(origin: Option<String>) -> ThemeExtServerContext {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/theme_ext/fixtures");
        let mut ctx = build_theme_extension_context(directory, 11, Some(9293));
        ctx.session = empty_dev_session("shop.myshopify.com");
        ctx.store_origin = origin;
        ctx
    }

    fn host_headers(host: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(host) = host {
            headers.insert(HOST, host.parse().unwrap());
        }
        headers
    }

    async fn send(router: Router, request: Request<Body>) -> (StatusCode, Vec<u8>) {
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, bytes.to_vec())
    }

    #[test]
    fn accepts_loopback_hosts() {
        let ctx = context_at(9293);
        for host in [
            "localhost:9293",
            "127.0.0.1:9293",
            "[::1]:9293",
            "LOCALHOST:9293",
            "localhost.:9293",
        ] {
            assert!(
                valid_extension_host(&ctx, &host_headers(Some(host))),
                "host={host}"
            );
        }
    }

    #[test]
    fn rejects_attacker_hosts_and_missing_host() {
        let ctx = context_at(9293);
        for host in [
            "attacker.com:9293",
            "poc.mzero.cloud:9293",
            "localhost:1234",
        ] {
            assert!(
                !valid_extension_host(&ctx, &host_headers(Some(host))),
                "host={host}"
            );
        }
        assert!(!valid_extension_host(&ctx, &host_headers(None)));
    }

    #[test]
    fn accepts_uppercase_bound_host_flag() {
        let mut ctx = context_at(9293);
        ctx.host = "LOCALHOST".into();
        assert!(valid_extension_host(
            &ctx,
            &host_headers(Some("localhost:9293"))
        ));
    }

    #[test]
    fn default_port_is_9293() {
        assert_eq!(DEFAULT_THEME_EXT_PORT, 9293);
        assert_eq!(DEFAULT_THEME_EXT_HOST, "127.0.0.1");
    }

    #[test]
    fn normalize_strips_trailing_dot_before_port() {
        assert_eq!(normalize_host_header("localhost.:9293"), "localhost:9293");
    }

    #[test]
    fn ignored_paths_include_wpm() {
        assert!(should_ignore("/wpm@something"));
        assert!(!should_ignore("/"));
    }

    #[tokio::test]
    async fn ignored_path_returns_204() {
        let router = theme_extension_router_from_context(fixture_context(None));
        let (status, _) = send(
            router,
            Request::builder()
                .uri("/.well-known/appspecific/com.chrome.devtools.json")
                .header(HOST, "127.0.0.1:9293")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn middleware_rejects_invalid_host_on_explicit_routes() {
        for uri in ["/__theme_dev/hot-reload", "/assets/thumbs-up.png", "/"] {
            let router = theme_extension_router_from_context(fixture_context(None));
            let (status, _) = send(
                router,
                Request::builder()
                    .uri(uri)
                    .header(HOST, "attacker.example:9293")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "uri={uri}");
        }
    }

    #[tokio::test]
    async fn html_path_hits_storefront_mock_and_injects_hot_reload() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><head></head><body>storefront</body></html>"),
            )
            .mount(&server)
            .await;
        let router = theme_extension_router_from_context(fixture_context(Some(server.uri())));
        let (status, body) = send(
            router,
            Request::builder()
                .uri("/")
                .header(HOST, "127.0.0.1:9293")
                .header("accept", "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        let body = String::from_utf8_lossy(&body);
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("storefront"));
        assert!(body.contains("hot-reload-client"));
    }

    #[tokio::test]
    async fn cdn_path_is_proxied() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cdn/foo.js"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/javascript")
                    .set_body_string("console.log(1)"),
            )
            .mount(&server)
            .await;
        let router = theme_extension_router_from_context(fixture_context(Some(server.uri())));
        let (status, body) = send(
            router,
            Request::builder()
                .uri("/cdn/foo.js")
                .header(HOST, "127.0.0.1:9293")
                .header("accept", "application/javascript")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(String::from_utf8_lossy(&body), "console.log(1)");
    }

    #[tokio::test]
    async fn asset_path_is_served_locally() {
        let router = theme_extension_router_from_context(fixture_context(None));
        let (status, body) = send(
            router,
            Request::builder()
                .uri("/assets/thumbs-up.png")
                .header(HOST, "127.0.0.1:9293")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn file_change_emits_hot_reload_update() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("snippets")).unwrap();
        let file = dir.path().join("snippets/hello.liquid");
        std::fs::write(&file, "hello").unwrap();
        let ctx = build_theme_extension_context(dir.path(), 11, Some(9293));
        let (_router, tx) = theme_extension_app(ctx.clone());
        let mut rx = tx.subscribe();
        ctx.filesystem
            .handle_file_update(ThemeExtFsEventName::Change, &file);
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match event {
            HotReloadEvent::Update { key, payload, .. } => {
                assert_eq!(key, "snippets/hello.liquid");
                assert!(payload.is_theme_extension);
            }
            other => panic!("expected update, got {other:?}"),
        }
    }
}
